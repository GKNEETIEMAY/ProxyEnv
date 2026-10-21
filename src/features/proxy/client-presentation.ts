import type { ProxyCandidate } from "../../shared/types";
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
export function useGenericIcon(event: Event) {
  const image = event.currentTarget as HTMLImageElement;
  const fallback = clientIcons["generic-proxy"];
  if (!image.src.endsWith(fallback)) image.src = fallback;
}
