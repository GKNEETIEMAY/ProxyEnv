import { onBeforeUnmount, onMounted, ref } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { ProxyEndpoint } from "../../shared/types";
export type BridgeStatus = "disconnected" | "connecting" | "connected" | "stale" | "unavailable" | "error";
export interface BridgeEndpoint { local: ProxyEndpoint; remotePort: number }
export interface BridgeSummary { status: BridgeStatus; alias: string | null; proxy: BridgeEndpoint | null; cc: BridgeEndpoint | null; activeProxyRevision: number | null; environment: string; codexConfigured: boolean; claudeConfigured: boolean; error: string | null }
export interface BridgeRequest { alias: string; proxyPort: number | null; ccPort: number | null; ccLocalPort: number; expectedRevision: number }
export interface ConfigPreview { id: string; tool: string; path: string; before: string; after: string; version: string; launch: string; alias:string; restore:boolean }
export const emptySummary = (): BridgeSummary => ({ status:"disconnected", alias:null, proxy:null, cc:null, activeProxyRevision:null, environment:"", codexConfigured:false, claudeConfigured:false, error:null });
export const targetLabel = (target:string) => target.startsWith("vscode:") ? `${target.slice(7)} · VS Code` : target;
export const remoteBackend = {
  targets: () => invoke<string[]>("remote_bridge_targets"),
  summary: () => invoke<BridgeSummary>("remote_bridge_summary"),
  check: (alias: string) => invoke<void>("remote_bridge_check", { alias }),
  detectCc: (localPort: number) => invoke<boolean>("remote_bridge_detect_cc", { localPort }),
  preview: (request: BridgeRequest) => invoke<BridgeSummary>("remote_bridge_preview", { request }),
  connect: (request: BridgeRequest) => invoke<BridgeSummary>("remote_bridge_connect", { request, confirmed:true }),
  disconnect: () => invoke<BridgeSummary>("remote_bridge_disconnect", { confirmed:true }),
  test: () => invoke<void>("remote_bridge_test"),
  configPreview: (tool: string) => invoke<ConfigPreview>("remote_bridge_config_preview", { tool }),
  configApply: (id: string) => invoke<void>("remote_bridge_config_apply", { id, confirmed:true }),
  configRestorePreview: (alias:string, tool:string) => invoke<ConfigPreview>("remote_bridge_config_restore_preview", { alias,tool }),
  configRestore: (id: string) => invoke<void>("remote_bridge_config_restore", { id, confirmed:true }),
  openVscode: (alias:string) => invoke<void>("remote_bridge_open_vscode",{alias}),
};
export function useRemoteBridge() {
  const summary = ref<BridgeSummary>(emptySummary());
  let timer: ReturnType<typeof setTimeout> | undefined;
  let disposed=false;
  async function refresh() {
    if (!isTauri()) return;
    try { const next=await remoteBackend.summary(); if (!disposed) summary.value=next; }
    catch { if (!disposed) summary.value={...summary.value,status:"unavailable"}; }
  }
  async function poll() { await refresh(); if (!disposed) timer=setTimeout(poll,2000); }
  onMounted(() => { void poll(); });
  onBeforeUnmount(() => { disposed=true; clearTimeout(timer); });
  return { summary, refresh };
}
