<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { ActiveProxyContext } from "../../../shared/types";
import type { RemoteBridgeCopy } from "../../../shared/i18n/remote-bridge";
import HelpTooltip from "../../../shared/components/HelpTooltip.vue";
import StatusIndicator from "../../../shared/components/StatusIndicator.vue";
import { remoteToolAdapters, type RemoteToolId } from "../tool-adapters";
import type { BridgeSummary, BridgeStatus, RemoteSkill, BridgeCapability } from "../state";
import RemoteToolIcon from "./RemoteToolIcon.vue";
import { bridgeEndpointLabel as endpoint, bridgeCapabilityState as capabilityState, overviewToolChecking, overviewToolLabel, overviewSkillStatus } from "../overview-presentation";

const props = defineProps<{
  copy: RemoteBridgeCopy;
  summary: BridgeSummary;
  activeProxy: ActiveProxyContext;
  skills: Array<{ name: string; agents: Array<{ tool: RemoteToolId; label: string; skill?: RemoteSkill }> }>;
  connected: boolean;
  busy: boolean;
  ccAvailable: boolean;
  capabilityPending?: BridgeCapability;
  skillPending?: string;
}>();
const emit = defineEmits<{ toggleCapability: [id: BridgeCapability, enabled: boolean]; toggleTool: [id: RemoteToolId, configured: boolean]; toggleSkill: [skill: RemoteSkill] }>();
const skillsExpanded = ref(false);
const skillSearch = ref("");
const filteredSkills = computed(() => {
  const query = skillSearch.value.trim().toLocaleLowerCase();
  return props.skills.filter((group) => group.name.toLocaleLowerCase().includes(query));
});
watch(() => props.summary.target?.id, () => {
  skillsExpanded.value = false;
  skillSearch.value = "";
});
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
const tools = computed(() => remoteToolAdapters.map((adapter) => ({ id: adapter.id, name: adapter.id === "codex" ? "Codex" : "Claude Code", inspection: adapter.inspect(props.summary), checking: overviewToolChecking(props.summary, adapter.id) })));
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
          <div class="bridge-tool-copy"><span><strong>{{ tool.name }}</strong><HelpTooltip pinnable :label="tool.name" :text="copy.rbOverviewToolHelp" /></span><StatusIndicator v-if="tool.checking" state="checking" :label="copy.rbCheckChecking" /><small v-else :class="{ 'needs-attention': tool.inspection.configured && (!summary.cc || summary.ccStatus !== 'connected' || (tool.id === 'claude' && ['conflict', 'remoteChanged', 'restartRequired', 'invalidLocalProfile', 'localChanged', 'remoteUnavailable'].includes(summary.claudeProfileState ?? ''))) }">{{ toolLabel(tool.id, tool.inspection.configured) }}</small></div>
          <input class="switch-input" type="checkbox" role="switch" :checked="tool.inspection.configured" :disabled="busy || tool.checking || !connected || (!tool.inspection.configured && summary.ccStatus !== 'connected')" :aria-busy="tool.checking" :aria-label="`${tool.name} · ${tool.checking ? copy.rbCheckChecking : tool.inspection.configured ? copy.rbAccessEnabled : copy.rbAccessDisabled}`" @click.prevent="emit('toggleTool', tool.id, tool.inspection.configured)" />
        </div>
      </div>
      <header class="bridge-skills-heading">
        <h3>{{ copy.rbSkillsTitle }}<HelpTooltip pinnable :label="copy.rbSkillsTitle" :text="`${copy.rbOverviewSkillsHelp} ${copy.rbOverviewSkillsManageHint}`" /></h3>
        <span>{{ copy.rbOverviewSkillsCount.replace('{count}', String(skills.length)) }} · {{ skillsSummary }}</span>
        <button class="secondary-action bridge-skills-manage" type="button" :class="{ active: skillsExpanded }" :aria-label="skillsExpanded ? copy.rbOverviewCollapse : copy.rbOverviewManage" :title="skillsExpanded ? copy.rbOverviewCollapse : copy.rbOverviewManage" :aria-expanded="skillsExpanded" aria-controls="bridge-skills-manager" @click="skillsExpanded = !skillsExpanded"><svg aria-hidden="true" viewBox="0 0 24 24"><path d="m9 3 1-1h4l1 1 .5 2 2 1 2-.5 1.5 2.5-.5 2 1 2-1 2 .5 2-1.5 2.5-2-.5-2 1-.5 2-1 1h-4l-1-1-.5-2-2-1-2 .5L3 16l.5-2-1-2 1-2L3 8l1.5-2.5 2 .5 2-1 .5-2Z"/><circle cx="12" cy="12" r="3"/></svg></button>
      </header>
      <div v-show="skillsExpanded" id="bridge-skills-manager" class="bridge-skills-manager">
        <label class="bridge-skills-search"><svg aria-hidden="true" viewBox="0 0 24 24"><circle cx="10" cy="10" r="6"/><path d="m15 15 5 5"/></svg><input v-model="skillSearch" type="search" :placeholder="copy.rbOverviewSkillsSearch" :aria-label="copy.rbOverviewSkillsSearch" maxlength="128" /></label>
        <div v-if="filteredSkills.length" class="bridge-skills-list" tabindex="0" :aria-label="copy.rbSkillsTitle">
          <table class="bridge-skills-table" :aria-label="copy.rbSkillsTitle">
            <colgroup><col class="bridge-skill-name-column" /><col /><col /></colgroup>
            <thead><tr><th scope="col">Skills</th><th v-for="tool in tools" :key="tool.id" scope="col"><span><RemoteToolIcon :tool="tool.id" />{{ tool.name }}</span></th></tr></thead>
            <tbody>
              <tr v-for="group in filteredSkills" :key="group.name" class="bridge-skill-row">
                <th scope="row"><strong>{{ group.name }}</strong></th>
                <td v-for="agent in group.agents" :key="agent.tool">
                  <div class="bridge-skill-agent">
                    <span class="bridge-skill-agent-copy"><StatusIndicator :state="agent.skill && skillPending === agent.skill.id ? 'checking' : skillStatus(agent.skill).state" :label="agent.skill && skillPending === agent.skill.id ? copy.rbCheckChecking : skillStatus(agent.skill).label" /></span>
                    <input class="switch-input bridge-skill-action" type="checkbox" role="switch" :checked="!!agent.skill?.enabled" :disabled="busy || !connected || !agent.skill" :aria-busy="!!agent.skill && skillPending === agent.skill.id" :aria-label="`${group.name} · ${agent.label} · ${agent.skill?.enabled ? copy.rbOverviewSkillUnsync : copy.rbOverviewSkillSync}`" @click.prevent="agent.skill && emit('toggleSkill', agent.skill)" />
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
        <p v-else class="bridge-skills-empty" role="status">{{ skills.length ? copy.rbOverviewSkillsNoResults : copy.rbSkillsEmpty }}</p>
        <p v-if="skills.some(group => group.agents.some(agent => agent.skill?.enabled))" class="bridge-skills-help">{{ copy.rbSkillRestart }}</p>
      </div>
    </section>
  </div>
</template>

<style scoped>
.bridge-overview { display:grid; min-width:0; gap:12px; }
.bridge-capabilities,.bridge-tool-grid { display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); gap:8px; }
.bridge-capability,.bridge-ai-section { min-width:0; border:1px solid var(--line); border-radius:12px; background:var(--surface-strong); }
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
.bridge-ai-section { padding:12px; }
.bridge-ai-section > h2 { display:flex; align-items:center; gap:8px; margin:0 0 8px; font-size:16px; font-weight:680; }
.bridge-ai-section > h3 { margin:0 0 8px; font-size:13px; }
.bridge-tool { display:flex; min-width:0; align-items:center; gap:8px; padding:8px 10px; border:1px solid var(--line); border-radius:10px; }
.bridge-tool :deep(.remote-tool-icon) { width:28px; height:28px; }
.bridge-tool-copy { display:grid; min-width:0; gap:3px; flex:1; }
.bridge-tool-copy > span { display:flex; align-items:center; gap:8px; }
.bridge-tool-copy strong { font-size:14px; }
.bridge-tool-copy small { color:var(--muted); font-size:11px; line-height:1.5; }
.bridge-tool-copy small.needs-attention { color:var(--warning); }
.bridge-skills-heading { display:flex; flex-wrap:wrap; align-items:center; gap:8px 12px; margin:12px 0 0; padding-top:12px; border-top:1px solid var(--line); }
.bridge-skills-heading h3 { display:flex; align-items:center; gap:8px; margin:0; font-size:14px; }
.bridge-skills-heading > span { margin-left:auto; color:var(--muted); font-size:11px; }
.bridge-skills-heading .bridge-skills-manage { display:inline-flex; align-items:center; justify-content:center; width:30px; min-height:30px; padding:6px; }
.bridge-skills-manage.active { border-color:var(--accent); background:var(--accent-soft); color:var(--accent-strong); }
.bridge-skills-heading svg { width:15px; height:15px; fill:none; stroke:currentColor; stroke-width:1.6; }
.bridge-skills-manager { margin-top:6px; }
.bridge-skills-help { margin:8px 0; color:var(--muted); font-size:11px; line-height:1.55; }
.bridge-skills-search { display:flex; align-items:center; gap:8px; margin:0 0 6px; padding:0 10px; border:1px solid var(--line); border-radius:8px; background:var(--surface); }
.bridge-skills-search:focus-within { outline:2px solid var(--focus); outline-offset:2px; }
.bridge-skills-search svg { flex:none; width:16px; height:16px; fill:none; stroke:var(--muted); stroke-width:1.6; }
.bridge-skills-search input { width:100%; min-width:0; height:32px; border:0; outline:none; background:transparent; font-size:12px; caret-color:var(--accent); }
.bridge-skills-search input::placeholder { color:var(--muted); }
.bridge-skills-list { max-height:220px; overflow-y:scroll; scrollbar-gutter:stable; overscroll-behavior:contain; scrollbar-width:thin; scrollbar-color:var(--line) transparent; }
.bridge-skills-list:hover { scrollbar-color:var(--line-strong) transparent; }
.bridge-skills-list::-webkit-scrollbar { width:6px; }
.bridge-skills-list::-webkit-scrollbar-button { display:none; width:0; height:0; }
.bridge-skills-list::-webkit-scrollbar-track { background:transparent; }
.bridge-skills-list::-webkit-scrollbar-thumb { background:var(--line); border-radius:6px; }
.bridge-skills-list:hover::-webkit-scrollbar-thumb { background:var(--line-strong); }
.bridge-skills-table { width:100%; table-layout:fixed; border-collapse:separate; border-spacing:0; text-align:left; }
.bridge-skill-name-column { width:34%; }
.bridge-skills-table th,.bridge-skills-table td { padding:7px 8px; border-bottom:1px solid var(--line); }
.bridge-skills-table th:first-child { padding-left:0; }
.bridge-skills-table td:last-child { padding-right:0; }
.bridge-skills-table thead th { position:sticky; top:0; z-index:1; background:var(--surface-strong); color:var(--muted); font-size:11px; font-weight:600; }
.bridge-skills-table thead span { display:flex; align-items:center; gap:5px; }
.bridge-skills-table thead :deep(.remote-tool-icon) { width:14px; height:14px; flex:none; }
.bridge-skill-row strong { font-size:12px; font-weight:620; overflow-wrap:anywhere; }
.bridge-skill-agent { display:flex; align-items:center; justify-content:space-between; gap:8px; min-width:0; }
.bridge-skill-agent-copy { min-width:0; }
.bridge-skill-agent :deep(.check-status) { font-size:10px; }
.bridge-skill-action { width:36px; height:20px; margin:0; }
.bridge-skill-action::before { width:12px; height:12px; }
.bridge-skill-action:checked::before { transform:translateX(16px); }
.bridge-skill-action:disabled { opacity:.5; }
.bridge-skills-empty { margin:12px 0 2px; color:var(--muted); font-size:12px; line-height:1.6; }
@media (max-width:1024px) {
  .bridge-mapping { margin-left:0; }
}
@media (max-width:760px) {
  .bridge-capabilities,.bridge-tool-grid { grid-template-columns:minmax(0,1fr); }
}
</style>
