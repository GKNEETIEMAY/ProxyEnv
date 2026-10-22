import { compareVersions, isOfficialReleaseUrl, stableVersion, type GitHubRelease } from "./update";

export type UpdateFailure = "rateLimit" | "timeout" | "network" | "tls" | "proxy" | "signature" | "invalidResponse" | "service" | "development";
export type UpdatePhase = "release" | "metadata" | "download" | "install";
export class UpdateRequestError extends Error {
  constructor(readonly category: UpdateFailure) { super(category); }
}

export async function fetchLatestRelease(fetcher: typeof fetch, signal: AbortSignal): Promise<GitHubRelease | null> {
  return fetchRelease(fetcher, signal, "latest");
}

export async function fetchInstalledRelease(fetcher: typeof fetch, signal: AbortSignal, version: string): Promise<GitHubRelease | null> {
  const normalized = stableVersion(version);
  if (!normalized) throw new UpdateRequestError("invalidResponse");
  const release = await fetchRelease(fetcher, signal, `tags/v${normalized}`);
  if (!release) return null;
  const resolved = resolveUpdate(normalized, { status: "fulfilled", value: release }, { status: "fulfilled", value: null }, false);
  if (resolved.state !== "latest" || resolved.version !== normalized) throw new UpdateRequestError("invalidResponse");
  return release;
}

async function fetchRelease(fetcher: typeof fetch, signal: AbortSignal, path: string): Promise<GitHubRelease | null> {
  const response = await fetcher(`https://api.github.com/repos/GKNEETIEMAY/ProxyEnv/releases/${path}`, {
    cache: "no-store", headers: { Accept: "application/vnd.github+json" }, signal,
  });
  if (response.status === 404) return null;
  if (response.status === 429 || (response.status === 403 && response.headers.get("x-ratelimit-remaining") === "0")) throw new UpdateRequestError("rateLimit");
  if (!response.ok) throw new UpdateRequestError("service");
  try {
    const value: unknown = await response.json();
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new UpdateRequestError("invalidResponse");
    return value as GitHubRelease;
  }
  catch { throw new UpdateRequestError("invalidResponse"); }
}

// Never return exception text: transport errors can contain URLs, tokens or local paths.
export function updateFailure(error: unknown): UpdateFailure {
  if (error instanceof UpdateRequestError) return error.category;
  const text = String(error instanceof Error ? `${error.name} ${error.message}` : error).toLowerCase();
  if (/signature|pubkey|public key|verification|verify.*sign/.test(text)) return "signature";
  if (/abort|timeout|timed out/.test(text)) return "timeout";
  if (/certificate|tls|ssl/.test(text)) return "tls";
  if (/proxy|407/.test(text)) return "proxy";
  if (/network|fetch|connect|dns|resolve|request/.test(text)) return "network";
  return "service";
}

export interface UpdateMetadata {
  version: string;
  body?: string;
  date?: string;
}

// A cold connection or briefly stale release asset must not require a second click.
// Each attempt uses the caller's timeout; never retry trust or validation failures.
export async function checkUpdateMetadata<T extends UpdateMetadata>(
  current: string,
  releaseRequest: Promise<GitHubRelease | null>,
  checker: () => Promise<T | null>,
): Promise<T | null> {
  let metadata: T | null;
  try { metadata = await checker(); }
  catch (cause) {
    if (!["network", "timeout"].includes(updateFailure(cause))) throw cause;
    return checker();
  }
  if (metadata) return metadata;
  const announcement = await releaseRequest.then(
    value => ({ status: "fulfilled" as const, value }),
    reason => ({ status: "rejected" as const, reason }),
  );
  const result = resolveUpdate(current, announcement, { status: "fulfilled", value: null }, true);
  return result.state === "manual" ? checker() : null;
}

export type Availability<T extends UpdateMetadata> = {
  state: "latest" | "available" | "manual" | "unpublished" | "conflict" | "error";
  version: string;
  url: string;
  body: string;
  date: string;
  handle: T | null;
  failure?: UpdateFailure;
  source?: "release" | "metadata";
};

export function resolveUpdate<T extends UpdateMetadata>(
  current: string,
  releaseResult: PromiseSettledResult<GitHubRelease | null>,
  metadataResult: PromiseSettledResult<T | null>,
  installSupported: boolean,
): Availability<T> {
  const empty: Availability<T> = { state: "error", version: "", url: "", body: "", date: "", handle: null };
  if (!stableVersion(current)) return { ...empty, failure: "invalidResponse" };
  const release = releaseResult.status === "fulfilled" ? releaseResult.value : null;
  const metadata = metadataResult.status === "fulfilled" ? metadataResult.value : null;
  const apiVersion = release?.tag_name ? stableVersion(release.tag_name) : null;
  const metadataVersion = metadata ? stableVersion(metadata.version) : null;
  if (release && (!apiVersion || release.draft || release.prerelease || !release.html_url
    || (release.body != null && typeof release.body !== "string")
    || (release.published_at != null && typeof release.published_at !== "string")
    || !isOfficialReleaseUrl(release.html_url)
    || new URL(release.html_url).pathname !== `/GKNEETIEMAY/ProxyEnv/releases/tag/${release.tag_name}`
    || Boolean(new URL(release.html_url).search || new URL(release.html_url).hash))) {
    return { ...empty, failure: "invalidResponse" };
  }
  if (metadata && !metadataVersion) return { ...empty, failure: "invalidResponse" };
  if (apiVersion && metadataVersion && apiVersion !== metadataVersion) return { ...empty, state: "conflict" };
  if (apiVersion && release) {
    const newer = compareVersions(apiVersion, current) > 0;
    return {
      state: !newer ? "latest" : metadata && installSupported ? "available" : "manual",
      version: apiVersion, url: release.html_url!, body: release.body ?? "", date: release.published_at ?? "",
      handle: newer && metadata && installSupported ? metadata : null, source: "release",
      ...(newer && installSupported && metadataResult.status === "rejected"
        ? { failure: updateFailure(metadataResult.reason) } : {}),
    };
  }
  // The official updater metadata can be used when the announcement service fails.
  // This does NOT mean the download has been cryptographically verified yet.
  if (metadata && metadataVersion && compareVersions(metadataVersion, current) > 0) {
    const complete = typeof metadata.body === "string" && Boolean(metadata.body.trim());
    return {
      state: installSupported && complete ? "available" : "manual", version: metadataVersion,
      url: `https://github.com/GKNEETIEMAY/ProxyEnv/releases/tag/v${metadataVersion}`,
      body: typeof metadata.body === "string" ? metadata.body : "", date: metadata.date ?? "", handle: installSupported && complete ? metadata : null, source: "metadata",
    };
  }
  if (releaseResult.status === "fulfilled" && releaseResult.value === null && metadataResult.status === "fulfilled") {
    return { ...empty, state: "unpublished" };
  }
  const failure = releaseResult.status === "rejected" ? updateFailure(releaseResult.reason)
    : metadataResult.status === "rejected" ? updateFailure(metadataResult.reason) : "invalidResponse";
  return { ...empty, failure };
}
