<script setup lang="ts">
import { computed } from "vue";
import type { ActiveProxyContext } from "../../../shared/types";
import type { RemoteBridgeCopy } from "../../../shared/i18n/remote-bridge";
import HelpTooltip from "../../../shared/components/HelpTooltip.vue";
import StatusIndicator from "../../../shared/components/StatusIndicator.vue";
import { remoteToolAdapters, type RemoteToolId } from "../tool-adapters";
import type { BridgeSummary, BridgeStatus, RemoteSkill, BridgeCapability } from "../state";
import RemoteToolIcon from "./RemoteToolIcon.vue";
import { bridgeEndpointLabel as endpoint, bridgeCapabilityState as capabilityState, overviewToolLabel, overviewSkillStatus } from "../overview-presentation";

const props = defineProps<{
  copy: RemoteBridgeCopy;
  summary: BridgeSummary;
  activeProxy: ActiveProxyContext;
  skills: Array<{ name: string; agents: Array<{ tool: RemoteToolId; label: string; skill?: RemoteSkill }> }>;
  connected: boolean;
  busy: boolean;
  ccAvailable: boolean;
  capabilityPending?: BridgeCapability;
}>();
const emit = defineEmits<{ toggleCapability: [id: BridgeCapability, enabled: boolean]; toggleTool: [id: RemoteToolId, configured: boolean]; manageSkills: []; terminal: []; vscode: []; mobaxterm: [] }>();
const capabilities = computed(() => [
  { id: "proxy" as const, title: props.copy.rbLocalNetworkTitle, help: props.copy.rbLocalNetworkHelp, route: props.summary.proxy, status: props.summary.proxyStatus, available: props.activeProxy.available },
  { id: "cc" as const, title: props.copy.rbCcUseTitle, help: props.copy.rbOverviewAiHelp, route: props.summary.cc, status: props.summary.ccStatus, available: props.ccAvailable },
]);
function capabilityLabel(status: BridgeStatus | null, enabled: boolean): string {
  if (!enabled) return props.copy.rbOverviewNotEnabled;
  return status === "connected" ? props.copy.rbAccessEnabled : status ? props.copy.rbStates[status] : props.copy.rbCheckIdle;
}
const proxyClient = computed(() => {
  const selected = props.activeProxy.candidate;
  const local = props.summary.proxy?.local;
  // A stale bridge still uses its original endpoint, not the newly selected local proxy.
  return selected && local && selected.host === local.host && selected.port === local.port && selected.protocol === local.protocol
    ? selected.clientName : props.copy.rbOverviewLocalProxy;
});
const tools = computed(() => remoteToolAdapters.map((adapter) => ({ id: adapter.id, name: adapter.id === "codex" ? "Codex" : "Claude Code", inspection: adapter.inspect(props.summary) })));
function toolLabel(id: RemoteToolId, configured: boolean): string {
  return overviewToolLabel(props.copy, props.summary, id, configured);
}
const skillStatus = (skill?: RemoteSkill) => overviewSkillStatus(props.copy, skill);
const skillsSummary = computed(() => {
  const enabled = props.skills.flatMap((group) => group.agents.flatMap((agent) => agent.skill?.enabled ? [agent.skill] : []));
  return props.copy.rbOverviewSkillsAuto.replace("{synced}", String(enabled.filter((skill) => skill.state === "synced").length)).replace("{total}", String(enabled.length));
});
</script>

<template>
  <div class="bridge-overview">
    <div class="bridge-capabilities">
      <section v-for="capability in capabilities" :key="capability.id" class="bridge-capability" :class="{ 'is-off': !capability.route }">
        <header>
          <svg class="section-icon" aria-hidden="true" viewBox="0 0 24 24"><template v-if="capability.id === 'proxy'"><path d="m6 17 6-11 6 11M6 17h12"/><circle cx="12" cy="5" r="2.5"/><circle cx="5" cy="18" r="2.5"/><circle cx="19" cy="18" r="2.5"/></template><path v-else d="M3 7h17m-5-5 5 5-5 5M21 17H4m5-5-5 5 5 5"/></svg>
          <h2>{{ capability.title }}</h2>
          <HelpTooltip pinnable :label="capability.title" :text="`${capability.help} ${copy.rbOverviewCapabilityHelp}`" />
          <StatusIndicator :state="capabilityPending === capability.id ? 'checking' : capabilityState(capability.status, !!capability.route)" :label="capabilityPending === capability.id ? copy.rbCheckChecking : capabilityLabel(capability.status, !!capability.route)" />
          <input class="switch-input capability-switch" type="checkbox" role="switch" :checked="!!capability.route" :disabled="busy || !connected || summary.postConnectStatus === 'preparing' || (!capability.route && !capability.available)" :aria-busy="capabilityPending === capability.id" :aria-label="`${capability.title} · ${capability.route ? copy.rbAccessEnabled : copy.rbAccessDisabled}`" @click.prevent="emit('toggleCapability', capability.id, !capability.route)" />
        </header>
        <div v-if="capability.route" class="bridge-mapping">
          <span class="bridge-mapping-local-label">{{ copy.rbOverviewLocalProxy }}</span>
          <code class="bridge-mapping-local-address">{{ endpoint(capability.route.local.host, capability.route.local.port) }}</code>
          <small class="bridge-mapping-client">{{ capability.id === 'cc' ? 'CC Switch' : `${proxyClient} (${capability.route.local.protocol})` }}</small>
          <svg class="bridge-mapping-arrow" aria-hidden="true" viewBox="0 0 24 24"><path d="M4 12h16m-6-6 6 6-6 6"/></svg>
          <span class="bridge-mapping-remote-label">{{ copy.rbRemotePort }}</span>
          <code class="bridge-mapping-remote-address">127.0.0.1:{{ capability.route.remotePort }}</code>
        </div>
        <p v-else class="bridge-mapping-empty">{{ copy.rbOverviewNotEnabled }}</p>
      </section>
    </div>

    <section class="bridge-ai-section" aria-labelledby="bridge-ai-heading">
      <h2 id="bridge-ai-heading"><svg class="section-icon" aria-hidden="true" viewBox="0 0 24 24"><path d="m12 2 3 7 7 3-7 3-3 7-3-7-7-3 7-3Z"/></svg>{{ copy.rbOverviewAiTitle }}</h2>
      <h3>{{ copy.rbOverviewAiTools }}</h3>
      <div class="bridge-tool-grid">
        <div v-for="tool in tools" :key="tool.id" class="bridge-tool">
          <RemoteToolIcon :tool="tool.id" />
          <div class="bridge-tool-copy"><span><strong>{{ tool.name }}</strong><HelpTooltip pinnable :label="tool.name" :text="copy.rbOverviewToolHelp" /></span><small :class="{ 'needs-attention': tool.inspection.configured && (!summary.cc || summary.ccStatus !== 'connected' || (tool.id === 'claude' && ['conflict', 'remoteChanged', 'restartRequired', 'invalidLocalProfile', 'localChanged', 'remoteUnavailable'].includes(summary.claudeProfileState ?? ''))) }">{{ toolLabel(tool.id, tool.inspection.configured) }}</small></div>
          <input class="switch-input" type="checkbox" role="switch" :checked="tool.inspection.configured" :disabled="busy || !connected || (!tool.inspection.configured && summary.ccStatus !== 'connected')" :aria-label="`${tool.name} · ${tool.inspection.configured ? copy.rbAccessEnabled : copy.rbAccessDisabled}`" @click.prevent="emit('toggleTool', tool.id, tool.inspection.configured)" />
        </div>
      </div>
      <header class="bridge-skills-heading">
        <h3>{{ copy.rbSkillsTitle }}<HelpTooltip pinnable :label="copy.rbSkillsTitle" :text="copy.rbOverviewSkillsHelp" /></h3>
        <span>{{ skillsSummary }}</span>
        <button class="secondary-action" type="button" :disabled="busy" @click="emit('manageSkills')"><svg aria-hidden="true" viewBox="0 0 24 24"><path d="M9 3h6l1 3 3 1 2 5-2 5-3 1-1 3H9l-1-3-3-1-2-5 2-5 3-1Z"/><circle cx="12" cy="12" r="3"/></svg>{{ copy.rbOverviewManage }}</button>
      </header>
      <div v-if="skills.length" class="bridge-skills-list">
        <div v-for="group in skills" :key="group.name" class="bridge-skill-row">
          <strong>{{ group.name }}</strong>
          <StatusIndicator v-for="agent in group.agents" :key="agent.tool" :state="skillStatus(agent.skill).state" :label="`${agent.label} ${skillStatus(agent.skill).label}`" />
        </div>
      </div>
      <p v-else class="bridge-skills-empty">{{ copy.rbSkillsEmpty }}</p>
    </section>

    <footer class="bridge-launch-bar">
      <button class="primary-action" type="button" :disabled="busy || !connected" @click="emit('terminal')"><svg aria-hidden="true" viewBox="0 0 24 24"><path d="m4 5 7 7-7 7m10 0h7"/></svg>{{ copy.rbOpenTerminal }}</button>
      <span class="bridge-launch-option"><button class="secondary-action" type="button" :disabled="busy || !connected || !summary.target?.canOpenVscode" @click="emit('vscode')"><svg aria-hidden="true" viewBox="0 0 24 24"><path d="m16 3 5 2v14l-5 2-11-9 11-9Z M16 3v18M5 6l-3 2 14 13M5 18l-3-2L16 3"/></svg>{{ copy.rbVscodeOpen }}</button><HelpTooltip v-if="!summary.target?.canOpenVscode" pinnable :label="copy.rbVscodeOpen" :text="copy.rbOverviewVscodeUnavailable" /></span>
      <span class="bridge-launch-option"><button class="secondary-action" type="button" :disabled="busy || !connected || !summary.target?.canOpenMobaxterm" @click="emit('mobaxterm')"><svg aria-hidden="true" viewBox="0 0 24 24"><path d="m4 5 7 7-7 7m10 0h7"/></svg>{{ copy.rbLaunchMobaxterm }}</button><HelpTooltip pinnable :label="copy.rbLaunchMobaxterm" :text="summary.target?.canOpenMobaxterm ? `${copy.rbMobaConnectionHint} ${copy.rbMobaProxyHint} ${copy.rbMobaAiHint}` : copy.rbOverviewMobaUnavailable" /></span>
    </footer>
  </div>
</template>

<style scoped>
.bridge-overview { display:grid; min-width:0; gap:14px; }
.bridge-capabilities,.bridge-tool-grid { display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); gap:12px; }
.bridge-capability,.bridge-ai-section,.bridge-launch-bar { min-width:0; border:1px solid var(--line); border-radius:12px; background:var(--surface-strong); }
.bridge-capability { padding:12px 16px; }
.bridge-capability header { display:flex; align-items:center; flex-wrap:wrap; gap:8px; }
.section-icon { width:25px; height:25px; color:var(--accent); fill:none; stroke:currentColor; stroke-width:1.8; stroke-linecap:round; stroke-linejoin:round; flex:none; }
.bridge-capability h2 { margin:0; font-size:14px; font-weight:680; }
.capability-switch { margin-left:auto; }
.bridge-mapping { display:grid; grid-template-columns:minmax(0,1fr) 22px minmax(0,1fr); grid-template-rows:auto auto auto; align-items:center; gap:3px 12px; margin:10px 0 2px 33px; color:var(--muted); font-size:11px; }
.bridge-mapping-local-label { grid-column:1; grid-row:1; }
.bridge-mapping-local-address { grid-column:1; grid-row:2; }
.bridge-mapping-client { grid-column:1; grid-row:3; }
.bridge-mapping-remote-label { grid-column:3; grid-row:1; }
.bridge-mapping-remote-address { grid-column:3; grid-row:2; }
.bridge-mapping span,.bridge-mapping small,.bridge-mapping code { font:inherit; overflow-wrap:anywhere; }
.bridge-mapping-arrow { grid-column:2; grid-row:2; width:22px; height:18px; fill:none; stroke:var(--muted); stroke-width:1.6; stroke-linecap:round; stroke-linejoin:round; }
.bridge-mapping-empty { margin:10px 0 2px 33px; color:var(--muted); font-size:11px; }
.is-off .section-icon { color:var(--muted); }
.bridge-ai-section { padding:16px; }
.bridge-ai-section > h2 { display:flex; align-items:center; gap:9px; margin:0 0 16px; font-size:18px; font-weight:680; }
.bridge-ai-section > h3 { margin:0 0 8px; font-size:13px; }
.bridge-tool { display:flex; min-width:0; align-items:center; gap:14px; padding:14px; border:1px solid var(--line); border-radius:10px; }
.bridge-tool-copy { display:grid; min-width:0; gap:5px; flex:1; }
.bridge-tool-copy > span { display:flex; align-items:center; gap:8px; }
.bridge-tool-copy strong { font-size:14px; }
.bridge-tool-copy small { color:var(--muted); font-size:11px; line-height:1.5; }
.bridge-tool-copy small.needs-attention { color:var(--warning); }
.bridge-skills-heading { display:flex; flex-wrap:wrap; align-items:center; gap:8px 12px; margin:16px 0 8px; }
.bridge-skills-heading h3 { display:flex; align-items:center; gap:8px; margin:0; font-size:14px; }
.bridge-skills-heading > span { margin-left:auto; color:var(--muted); font-size:11px; }
.bridge-skills-heading button { display:inline-flex; align-items:center; gap:6px; min-height:30px; padding:5px 9px; font-size:11px; }
.bridge-skills-heading svg { width:15px; height:15px; fill:none; stroke:currentColor; stroke-width:1.6; }
.bridge-skills-list { display:grid; max-height:200px; gap:6px; overflow-y:auto; scrollbar-width:thin; scrollbar-color:var(--line) transparent; }
.bridge-skill-row { display:grid; grid-template-columns:minmax(0,1fr) minmax(130px,.55fr) minmax(130px,.55fr); align-items:center; gap:8px; padding:8px 10px; border:1px solid var(--line); border-radius:8px; }
.bridge-skill-row > strong { min-width:0; font-size:12px; font-weight:620; overflow-wrap:anywhere; }
.bridge-skill-row :deep(.check-status) { justify-content:center; padding:5px 8px; border-radius:7px; background:color-mix(in srgb,var(--muted) 6%,transparent); }
.bridge-skill-row :deep(.check-status[data-state="healthy"]) { background:var(--success-soft); }
.bridge-skill-row :deep(.check-status[data-state="failed"]) { background:var(--danger-soft); }
.bridge-skill-row :deep(.check-status[data-state="warning"]) { background:var(--warning-soft); }
.bridge-skills-empty { margin:12px 0 2px; color:var(--muted); font-size:12px; line-height:1.6; }
.bridge-launch-bar { position:sticky; bottom:0; z-index:2; display:flex; flex-wrap:wrap; align-items:center; gap:10px; padding:10px; }
.bridge-launch-option { display:inline-flex; align-items:center; gap:6px; }
.bridge-launch-bar button { display:inline-flex; align-items:center; justify-content:center; gap:10px; min-height:40px; font-size:12px; }
.bridge-launch-bar svg { width:20px; height:20px; fill:none; stroke:currentColor; stroke-width:1.8; stroke-linecap:round; stroke-linejoin:round; }
@media (max-width:1024px) {
  .bridge-mapping { margin-left:0; }
}
@media (max-width:760px) {
  .bridge-capabilities,.bridge-tool-grid { grid-template-columns:minmax(0,1fr); }
  .bridge-launch-bar button { min-height:36px; font-size:11px; gap:7px; }
}
@media (max-width:600px) {
  .bridge-skill-row { grid-template-columns:repeat(2,minmax(0,1fr)); }
  .bridge-skill-row > strong { grid-column:1 / -1; }
}
</style>
