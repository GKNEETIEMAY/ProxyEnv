import type { RemoteTarget } from "./state";

/** A display label, never a shell command. Keep custom SSH ports explicit. */
export function sshTargetAddress(target: RemoteTarget): string {
  if (!target.host) return target.sshAlias ?? target.displayName;
  const host = target.host.includes(":") ? `[${target.host}]` : target.host;
  const destination = `${target.user ? `${target.user}@` : ""}${host}`;
  return target.port && target.port !== 22 ? `${destination}:${target.port}` : destination;
}

/** Unknown keys require user confirmation; changed keys must not be accepted. */
export function needsInteractiveSshAuth(code: string): boolean {
  return code === "sshAuth" || code === "hostKey";
}
