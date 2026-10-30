import { compareVersions, stableVersion } from "./update";

export type UpdateFailure = "rateLimit" | "timeout" | "network" | "tls" | "proxy" | "signature" | "invalidResponse" | "service" | "development";
const failures: UpdateFailure[] = ["rateLimit", "timeout", "network", "tls", "proxy", "signature", "invalidResponse", "service", "development"];
export class UpdateRequestError extends Error {
  constructor(readonly category: UpdateFailure, readonly retryAt?: number) { super(category); }
}
// Never display raw exception text: it can contain URLs, tokens or local paths.
export function updateFailure(error: unknown): UpdateFailure {
  if (error && typeof error === "object" && "category" in error && failures.includes(error.category as UpdateFailure)) return error.category as UpdateFailure;
  const text = String(error instanceof Error ? `${error.name} ${error.message}` : error).toLowerCase();
  if (/signature|pubkey|public key|verification|verify.*sign/.test(text)) return "signature";
  if (/abort|timeout|timed out/.test(text)) return "timeout";
  if (/certificate|tls|ssl/.test(text)) return "tls";
  if (/proxy|407|429|rate.?limit/.test(text)) return /429|rate.?limit/.test(text) ? "rateLimit" : "proxy";
  if (/network|fetch|connect|dns|resolve|request/.test(text)) return "network";
  return "service";
}
export interface UpdateMetadata { version: string; body: string; date: string }
export type Availability = {
  state: "latest" | "ahead" | "available" | "manual" | "unpublished" | "conflict" | "error";
  version: string; url: string; body: string; date: string;
  failure?: UpdateFailure;
  cached?: boolean;
};
export function validMetadata(value: unknown): value is UpdateMetadata {
  if (!value || typeof value !== "object") return false;
  const entry = value as UpdateMetadata;
  return Boolean(stableVersion(entry.version)) && typeof entry.body === "string" && entry.body.length <= 80_000
    && typeof entry.date === "string" && entry.date.length <= 64 && Number.isFinite(Date.parse(entry.date));
}
export function resolveUpdate(current: string, metadata: UpdateMetadata | null, installSupported: boolean): Availability {
  const empty: Availability = { state: "error", version: "", url: "", body: "", date: "" };
  if (!stableVersion(current) || (metadata !== null && !validMetadata(metadata))) return { ...empty, failure: "invalidResponse" };
  if (!metadata) return { ...empty, state: "unpublished" };
  const version = stableVersion(metadata.version)!;
  const comparison = compareVersions(version, current);
  return {
    ...metadata, version, url: `https://github.com/GKNEETIEMAY/ProxyEnv/releases/tag/v${version}`,
    state: comparison < 0 ? "ahead" : comparison === 0 ? "latest" : installSupported && metadata.body.trim() ? "available" : "manual",
  };
}

const CACHE_KEY = "proxyenv.updateMetadata.v1";
const TTL = 6 * 60 * 60 * 1000;
const MANUAL_INTERVAL = 30_000;
const MAX_COOLDOWN = 7 * 24 * 60 * 60 * 1000;
type Cache = { checkedAt: number; metadata: UpdateMetadata | null; retryAt: number };
type Result = { metadata: UpdateMetadata | null; cached: boolean };
type Storage = Pick<globalThis.Storage, "getItem" | "setItem">;

// Cache is display evidence only. No installer handles, URLs or signatures persist.
export class UpdateMetadataClient {
  private cache: Cache | null = null;
  private pending: Promise<Result> | null = null;
  constructor(private readonly fetcher: () => Promise<UpdateMetadata | null>, private readonly storage?: Storage, private readonly now = Date.now) {
    try {
      const raw = storage?.getItem(CACHE_KEY);
      if (!raw || raw.length > 100_000) return;
      const value = JSON.parse(raw) as Cache;
      const now = this.now();
      if (Number.isFinite(value.checkedAt) && value.checkedAt >= 0 && value.checkedAt <= now
        && Number.isFinite(value.retryAt) && value.retryAt >= 0 && value.retryAt <= now + MAX_COOLDOWN
        && (value.metadata === null || validMetadata(value.metadata))) this.cache = value;
    } catch { /* Cache/storage failure must not prevent a live check. */ }
  }
  get(mode: "automatic" | "manual" | "install" = "automatic"): Promise<Result> {
    if (this.pending) return this.pending;
    const now = this.now();
    if (this.cache && this.cache.retryAt > now) return Promise.reject(new UpdateRequestError("rateLimit", this.cache.retryAt));
    const ttl = mode === "automatic" && this.cache?.metadata ? TTL : MANUAL_INTERVAL;
    if (mode !== "install" && this.cache?.checkedAt && now - this.cache.checkedAt < ttl) {
      return Promise.resolve({ metadata: this.cache.metadata, cached: true });
    }
    this.pending = this.fetch().finally(() => { this.pending = null; });
    return this.pending;
  }
  private persist() {
    try { this.storage?.setItem(CACHE_KEY, JSON.stringify(this.cache)); } catch { /* In-memory cache still works. */ }
  }
  rememberFailure(cause: unknown): unknown {
    if (updateFailure(cause) !== "rateLimit") return cause;
    const now = this.now();
    const supplied = cause && typeof cause === "object" && "retryAt" in cause ? cause.retryAt : null;
    const retryAt = typeof supplied === "number" && Number.isFinite(supplied) ? Math.max(now + 60_000, Math.min(now + MAX_COOLDOWN, supplied)) : now + 60_000;
    this.cache = { checkedAt: 0, metadata: null, retryAt };
    this.persist();
    return new UpdateRequestError("rateLimit", retryAt);
  }
  private async fetch(): Promise<Result> {
    try {
      const metadata = await retryTransport(this.fetcher);
      if (metadata !== null && !validMetadata(metadata)) throw new UpdateRequestError("invalidResponse");
      this.cache = { checkedAt: this.now(), retryAt: 0, metadata: metadata && { version: metadata.version, body: metadata.body, date: metadata.date } };
      this.persist();
      return { metadata: this.cache.metadata, cached: false };
    } catch (cause) {
      throw this.rememberFailure(cause);
    }
  }
}

export async function retryTransport<T>(checker: () => Promise<T>): Promise<T> {
  try { return await checker(); }
  catch (cause) {
    if (!["network", "timeout"].includes(updateFailure(cause))) throw cause;
    return checker();
  }
}
// Default Tauri comparison stays upgrade-only. A cached announcement never authorizes installation.
export async function verifiedInstallHandle<T extends { version: string; close(): Promise<void> }>(
  current: string, expected: string, checker: () => Promise<T | null>,
): Promise<T | null> {
  if (!stableVersion(current) || !stableVersion(expected) || compareVersions(expected, current) <= 0) throw new UpdateRequestError("invalidResponse");
  let handle = await retryTransport(checker);
  if (!handle) handle = await checker();
  if (handle && stableVersion(handle.version) !== stableVersion(expected)) {
    await handle.close();
    throw new UpdateRequestError("invalidResponse");
  }
  return handle;
}
