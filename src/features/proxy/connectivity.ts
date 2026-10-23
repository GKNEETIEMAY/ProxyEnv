import type { ActiveProxyContext, CheckState, ProxyConnectivityResult } from "../../shared/types";

export const connectivityTtlMs = 30 * 60_000;
export function connectivityExpired(result: ProxyConnectivityResult, now = Date.now()): boolean {
  const testedAt = Date.parse(result.testedAt);
  return !Number.isFinite(testedAt) || now < testedAt || now - testedAt >= connectivityTtlMs;
}
export function connectivityTone(result: ProxyConnectivityResult | null, expired: boolean): CheckState {
  if (!result || expired) return "idle";
  if (!result.localListenerOk) return "failed";
  const passed = result.targets.filter(target => target.success).length;
  if (passed === result.targets.length && passed > 0) return "healthy";
  return passed > 0 || !result.targets.length ? "warning" : "failed";
}

export interface ConnectivityView {
  result: ProxyConnectivityResult | null;
  testing: boolean;
  cached: boolean;
  failed: boolean;
}

/** A single view generation guards both cache reads and tests, including A → B → A. */
export class ConnectivitySession {
  private generation = 0;
  private nextAutomaticTestAt = 0;
  constructor(
    readonly state: ConnectivityView,
    private readonly context: () => ActiveProxyContext,
    private readonly api: {
      testCurrentProxyConnectivity: (revision: number) => Promise<ProxyConnectivityResult>;
      lastProxyConnectivity: () => Promise<ProxyConnectivityResult | null>;
    },
    private readonly onError: (cause: unknown) => void,
    private readonly now: () => number = Date.now
  ) {}

  invalidate() {
    this.generation++;
    this.nextAutomaticTestAt = 0;
    Object.assign(this.state, { result: null, testing: false, cached: false, failed: false });
  }

  private accepts(generation: number, context: ActiveProxyContext, result?: ProxyConnectivityResult | null) {
    const current = this.context();
    return generation === this.generation && current.available
      && current.revision === context.revision && current.selectedCandidateId === context.selectedCandidateId
      && (!result || !!current.candidate && result.endpoint.host === current.candidate.host
        && result.endpoint.port === current.candidate.port && result.endpoint.protocol === current.candidate.protocol);
  }

  async readCache() {
    if (this.state.testing || !this.context().available) return;
    const generation = ++this.generation;
    const context = { ...this.context() };
    try {
      const result = await this.api.lastProxyConnectivity();
      if (this.accepts(generation, context, result)) {
        this.state.result = result;
        this.state.cached = result !== null;
      }
    } catch {
      // Cache is optional. A failed read must not masquerade as a failed network test.
    }
  }

  async testIfDue() {
    const now = this.now();
    if (now < this.nextAutomaticTestAt || this.state.testing || !this.context().available) return;
    if (this.state.result && !connectivityExpired(this.state.result, now)) return;
    await this.test(true);
  }

  async test(silent = false) {
    if (this.state.testing || !this.context().available) return;
    this.nextAutomaticTestAt = this.now() + connectivityTtlMs;
    const generation = ++this.generation;
    const context = { ...this.context() };
    Object.assign(this.state, { result: null, testing: true, cached: false, failed: false });
    try {
      const result = await this.api.testCurrentProxyConnectivity(context.revision);
      if (this.accepts(generation, context, result)) this.state.result = result;
    } catch (cause) {
      if (this.accepts(generation, context)) {
        this.state.failed = true;
        if (!silent) this.onError(cause);
      }
    } finally {
      if (generation === this.generation) this.state.testing = false;
    }
  }
}
