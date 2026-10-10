<script setup lang="ts">
import type { RemoteBridgeCopy } from "../../../shared/i18n/remote-bridge";
import type { RemoteTarget } from "../state";
import HelpTooltip from "../../../shared/components/HelpTooltip.vue";

defineProps<{ copy: RemoteBridgeCopy; target: RemoteTarget | null; connected: boolean; busy: boolean; canCopy: boolean; copying: boolean; copied: boolean }>();
const emit = defineEmits<{ terminal: []; vscode: []; mobaxterm: []; copyEnvironment: [] }>();
</script>

<template>
  <footer class="bridge-launch-bar">
    <button class="primary-action" type="button" :disabled="busy || !connected" @click="emit('terminal')"><img src="/remote-apps/powershell.png" alt="" />{{ copy.rbOpenTerminal }}</button>
    <span class="bridge-launch-option"><button class="secondary-action" type="button" :disabled="busy || !connected || !target?.canOpenVscode" @click="emit('vscode')"><img src="/remote-apps/vscode.ico" alt="" />{{ copy.rbVscodeOpen }}</button><HelpTooltip v-if="!target?.canOpenVscode" pinnable :label="copy.rbVscodeOpen" :text="copy.rbOverviewVscodeUnavailable" /></span>
    <span class="bridge-launch-option"><button class="secondary-action" type="button" :disabled="busy || !connected || !target?.canOpenMobaxterm" @click="emit('mobaxterm')"><img src="/remote-apps/mobaxterm.ico" alt="" />{{ copy.rbLaunchMobaxterm }}</button><HelpTooltip pinnable :label="copy.rbLaunchMobaxterm" :text="target?.canOpenMobaxterm ? `${copy.rbMobaConnectionHint} ${copy.rbMobaProxyHint} ${copy.rbMobaAiHint}` : copy.rbOverviewMobaUnavailable" /></span>
    <button class="secondary-action bridge-launch-copy" type="button" :class="{ 'is-copied': copied }" :disabled="busy || copying || !connected || !canCopy" :aria-busy="copying" :aria-label="copied ? copy.rbCopied : copy.rbCopySessionEnvironment" :title="canCopy ? copy.rbCopySessionEnvironment : `${copy.rbLocalNetworkTitle} · ${copy.rbOverviewNotEnabled}`" @click="emit('copyEnvironment')"><svg aria-hidden="true" viewBox="0 0 24 24"><path v-if="copied" d="m5 12 4 4L19 6"/><template v-else><rect x="8" y="8" width="12" height="13" rx="2"/><path d="M15 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h3"/></template></svg>{{ copy.rbCopySessionEnvironment }}</button>
  </footer>
</template>

<style scoped>
.bridge-launch-bar { flex:none; display:flex; flex-wrap:wrap; align-items:center; gap:8px; padding:12px 0 0; margin-top:12px; border-top:1px solid var(--line); background:var(--surface); }
.bridge-launch-option { display:inline-flex; align-items:center; gap:6px; min-width:0; }
.bridge-launch-bar button { display:inline-flex; align-items:center; justify-content:center; gap:8px; min-height:36px; font-size:12px; }
.bridge-launch-bar img { flex:none; width:20px; height:20px; object-fit:contain; }
.bridge-launch-bar button:disabled img { opacity:.5; }
.bridge-launch-copy { margin-left:auto; }
.bridge-launch-copy svg { flex:none; width:16px; height:16px; fill:none; stroke:currentColor; stroke-width:1.7; stroke-linecap:round; stroke-linejoin:round; }
.bridge-launch-copy.is-copied { color:var(--success); }
@media (max-width:760px) {
  .bridge-launch-bar { gap:6px; }
  .bridge-launch-bar button { min-height:34px; padding:7px 10px; font-size:11px; gap:6px; }
}
</style>
