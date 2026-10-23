import type { ManagedProxyVariable, ProxyCandidate, ProxyProtocol } from "../../shared/types";
const clientIcons: Record<string, string> = {
  "clash-verge-rev": "/proxy-clients/clash-verge-rev.png",
  v2rayn: "/proxy-clients/v2rayn.png",
  flclash: "/proxy-clients/flclash.ico",
  hiddify: "/proxy-clients/hiddify.ico",
  "clash-nyanpasu": "/proxy-clients/clash-nyanpasu.png",
  "mihomo-party": "/proxy-clients/mihomo-party.png",
  nekobox: "/proxy-clients/nekobox.png",
  "clash-for-windows": "/proxy-clients/clash-for-windows.png",
  "gui-for-clash": "/proxy-clients/gui-for-clash.png",
  "generic-proxy": "/proxy-clients/generic-proxy.svg"
};

export function candidateIcon(candidate?: ProxyCandidate): string {
  return clientIcons[candidate?.iconKey ?? ""] ?? clientIcons["generic-proxy"];
}
export function candidateEndpoint(candidate: ProxyCandidate): string {
  const host = candidate.host.includes(":") && !candidate.host.startsWith("[") ? `[${candidate.host}]` : candidate.host;
  return `${host}:${candidate.port}`;
}
export function candidateUsable(candidate: ProxyCandidate): boolean {
  return candidate.listening && candidate.protocol !== "unknown";
}
export function supportsProxyVariable(protocol: ProxyProtocol, variable: ManagedProxyVariable): boolean {
  return protocol === "mixed" || (variable === "all" ? protocol === "socks5" : protocol === "http");
}

// Detection order is the backend's recommendation order. Keep every endpoint
// selectable, but present recognized clients only once in the default list.
export function groupProxyCandidates(candidates: ProxyCandidate[], selectedId: string | null) {
  const groups = new Map<string, ProxyCandidate[]>();
  for (const candidate of candidates) {
    const known = candidate.iconKey && candidate.iconKey !== "generic-proxy" && clientIcons[candidate.iconKey];
    const key = known ? `client:${candidate.iconKey}:${candidate.clientName ?? ""}`
      : candidate.pid != null ? `process:${candidate.pid}:${candidate.processName ?? ""}` : `endpoint:${candidate.id}`;
    const endpoints = groups.get(key) ?? [];
    endpoints.push(candidate);
    groups.set(key, endpoints);
  }
  return [...groups].map(([key, endpoints]) => ({
    key, endpoints,
    candidate: endpoints.find(candidate => candidate.id === selectedId)
      ?? endpoints.find(candidateUsable) ?? endpoints[0]
  }));
}
export function useGenericIcon(event: Event) {
  const image = event.currentTarget as HTMLImageElement;
  const fallback = clientIcons["generic-proxy"];
  if (!image.src.endsWith(fallback)) image.src = fallback;
}
