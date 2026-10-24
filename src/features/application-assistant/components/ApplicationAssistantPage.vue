<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import type { Copy } from "../../../shared/i18n";
import type { ActiveProxyContext, CheckState, EnvironmentStatus, ManagedApplication, RunningApplication } from "../../../shared/types";
import { backend } from "../../../shared/api/backend";
import { withoutWindowsExtendedPathPrefix } from "../../../shared/utils/path";
import { copyText } from "../../../shared/utils/clipboard";
import { useNotification } from "../../../shared/state/notifications";
import HelpTooltip from "../../../shared/components/HelpTooltip.vue";
import StatusIndicator from "../../../shared/components/StatusIndicator.vue";

const props = defineProps<{ copy: Copy; reviewPreview?: boolean; activeProxyContext: ActiveProxyContext; environment: Pick<EnvironmentStatus, "state" | "entries" | "matchesActiveProxy">; stale: boolean }>();
const emit = defineEmits<{ selectionChange: [applicationId: string | undefined]; backLocal: [] }>();
const candidate = computed(() => props.activeProxyContext.candidate);
const protocols = computed(() => candidate.value?.protocol === "mixed" ? ["http", "socks5"] : candidate.value?.protocol === "http" || candidate.value?.protocol === "socks5" ? [candidate.value.protocol] : []);
const available = computed(() => !props.stale && props.activeProxyContext.available && Boolean(candidate.value?.listening) && protocols.value.length > 0);
const environmentStatus = computed<{ title: string; hint: string; tone: CheckState }>(() => {
  if (props.stale || !props.environment.entries.length) return { title: props.copy.appProxyStatusPending, hint: props.copy.appProxyStatusPendingHint, tone: "idle" };
  if (!available.value) return { title: props.copy.activeProxyUnavailableTitle, hint: props.copy.appProxyUnavailable, tone: "warning" };
  if (props.environment.state === "enabled" && props.environment.matchesActiveProxy) return { title: props.copy.appProxyStatusConfigured, hint: props.copy.appProxyStatusConfiguredHint, tone: "healthy" };
  if (props.environment.state === "disabled") return { title: props.copy.appProxyStatusUnset, hint: props.copy.appProxyStatusUnsetHint, tone: "idle" };
  return { title: props.copy.appProxyStatusUnsynced, hint: props.copy.appProxyStatusUnsyncedHint, tone: "warning" };
});
const manualHelp = computed(() => `${props.copy.appProxySourceHint.replace('{name}', candidate.value?.clientName || candidate.value?.processName || props.copy.localProxy)} ${props.copy.appProxyFieldsHelp} ${props.copy.appProxyManualHint}`);
const protocol = ref("http");
watch(protocols, values => { if (!values.includes(protocol.value)) protocol.value = values[0] ?? ""; }, { immediate: true });
const host = computed(() => candidate.value?.host.replace(/^\[(.*)\]$/, "$1") ?? "");
const fullAddress = computed(() => candidate.value ? `${protocol.value}://${host.value.includes(":") ? `[${host.value}]` : host.value}:${candidate.value.port}` : "");
const fields = computed(() => [
  { key: "host", label: props.copy.assistantHostAddress, value: host.value },
  { key: "port", label: props.copy.port, value: String(candidate.value?.port ?? "") },
]);
const copiedField = ref("");
const copyError = ref("");
useNotification(copyError, "error", "assistant-copy");
let copyTimer: ReturnType<typeof setTimeout> | undefined;
let copyGeneration = 0;
function clearCopy() {
  copyGeneration++;
  copiedField.value = "";
  copyError.value = "";
  if (copyTimer) clearTimeout(copyTimer);
}
async function copyField(key: string, value: string) {
  if (!available.value) return;
  clearCopy();
  const generation = copyGeneration;
  try {
    await copyText(value);
    if (generation !== copyGeneration) return;
    copiedField.value = key;
    copyTimer = setTimeout(() => { copiedField.value = ""; }, 1600);
  } catch { if (generation === copyGeneration) copyError.value = props.copy.copyFailed; }
}
watch([() => props.activeProxyContext.revision, protocol], clearCopy);

const expanded = ref(false);
const choosing = ref(true);
const applications = ref<RunningApplication[]>([]);
const loadingApps = ref(false);
const search = ref("");
const searchInput = ref<HTMLInputElement>();
const selected = ref<ManagedApplication>();
const launchButton = ref<HTMLButtonElement>();
const busy = ref(false);
const error = ref("");
useNotification(error, "error", "assistant-operation");
const launchedPid = ref<number>();
const filteredApplications = computed(() => {
  const query = search.value.trim().toLocaleLowerCase();
  return applications.value.filter(app => !query || `${app.displayName} ${app.executablePath ?? ""}`.toLocaleLowerCase().includes(query));
});
watch(() => selected.value?.id, id => emit("selectionChange", id));
watch(() => props.activeProxyContext.revision, () => { launchedPid.value = undefined; error.value = ""; });
onBeforeUnmount(() => { clearCopy(); emit("selectionChange", undefined); });

function selectApplication(application: ManagedApplication) {
  selected.value = application;
  choosing.value = false;
  error.value = "";
  launchedPid.value = undefined;
  void nextTick(() => launchButton.value?.focus());
}
function chooseRunning(app: RunningApplication) {
  if (busy.value || !app.applicationId || !app.executablePath) return;
  selectApplication({ id: app.applicationId, displayName: app.displayName, executablePath: app.executablePath });
}
async function loadApplications() {
  if (loadingApps.value || busy.value) return;
  loadingApps.value = true;
  error.value = "";
  try {
    applications.value = props.reviewPreview ? [
      { pid: 8420, applicationId: "review-code", processName: "Code.exe", displayName: "Visual Studio Code", executablePath: "C:\\Program Files\\Microsoft VS Code\\Code.exe", iconAvailable: false },
    ] : await backend.runningApplications();
  } catch (cause) { error.value = props.copy.appProxyLoadFailed.replace("{error}", String(cause)); }
  finally { loadingApps.value = false; }
}
async function togglePicker() {
  if (busy.value) return;
  expanded.value = !expanded.value;
  if (!expanded.value) return;
  if (selected.value) return;
  await openChoices();
}
async function openChoices() {
  if (busy.value) return;
  choosing.value = true;
  launchedPid.value = undefined;
  await loadApplications();
  await nextTick();
  searchInput.value?.focus();
}
async function browseApplication() {
  if (busy.value || props.reviewPreview) return;
  busy.value = true;
  error.value = "";
  try {
    const application = await backend.pickApplication();
    if (application) selectApplication(application);
  } catch (cause) { error.value = props.copy.appProxyLoadFailed.replace("{error}", String(cause)); }
  finally { busy.value = false; }
}

// Renew permission at the explicit launch action; opening this page does not enumerate or inspect apps.
async function authorizeApplication(application: ManagedApplication): Promise<ManagedApplication> {
  try { return await backend.renewApplicationAuthorization(application.id); }
  catch (cause) {
    const message = String(cause).toLocaleLowerCase("en-US");
    const expired = message.includes("application authorization is missing or expired");
    const unsupported = message.includes("command renew_application_authorization not found") || message.includes("unknown command") && message.includes("renew_application_authorization");
    if (!expired && !unsupported) throw cause;
    const refreshed = await backend.runningApplications();
    applications.value = refreshed;
    const normalize = (path: string) => withoutWindowsExtendedPathPrefix(path).replace(/\\/g, "/").toLocaleLowerCase("en-US");
    const match = refreshed.find(app => app.applicationId && app.executablePath && normalize(app.executablePath) === normalize(application.executablePath));
    if (match) return { ...application, id: match.applicationId!, executablePath: match.executablePath! };
    if (unsupported) return application;
    throw props.copy.assistantAuthorizationExpired;
  }
}
async function launch() {
  if (!selected.value || !available.value || busy.value) return;
  const revision = props.activeProxyContext.revision;
  const application = selected.value;
  busy.value = true;
  error.value = "";
  launchedPid.value = undefined;
  try {
    const authorized = props.reviewPreview ? application : await authorizeApplication(application);
    if (revision !== props.activeProxyContext.revision) throw props.copy.activeProxyChanged;
    if (!available.value) throw props.copy.appProxyUnavailable;
    selected.value = authorized;
    const result = props.reviewPreview ? { pid: 12480 } : await backend.launchApplicationWithProxy(authorized.id, revision);
    launchedPid.value = result.pid;
  } catch (cause) {
    const detail = String(cause);
    error.value = props.copy.appProxyLaunchFailed.replace("{error}", detail.includes("the active proxy changed") ? props.copy.activeProxyChanged : detail.includes("the current active proxy is unavailable") ? props.copy.appProxyUnavailable : detail);
  } finally { busy.value = false; }
}
</script>

<template>
  <main class="page workspace-page application-proxy-page">
    <section class="workspace-panel app-proxy-workspace">
      <header class="app-proxy-heading"><div class="app-proxy-title-help"><h1>{{ copy.appProxyTitle }}</h1><HelpTooltip :label="copy.appProxyTitle" :text="copy.appProxyRoutingHelp" pinnable /></div></header>
      <div class="app-proxy-content" tabindex="0" :aria-label="copy.appProxyTitle" role="region">
        <section class="app-proxy-status">
          <div class="app-proxy-title-help"><StatusIndicator :state="environmentStatus.tone" :label="environmentStatus.title" /><HelpTooltip :label="environmentStatus.title" :text="copy.appProxyEnvironmentHelp" pinnable /></div>
          <p class="app-proxy-note">{{ environmentStatus.hint }}</p>
          <button v-if="!available" class="secondary-action app-proxy-back" type="button" @click="emit('backLocal')">{{ copy.assistantBackLocal }}</button>
        </section>
        <section v-if="available" class="app-proxy-manual">
          <header class="app-proxy-section-heading">
            <div class="app-proxy-title-help"><h2>{{ copy.appProxyManualTitle }}</h2><HelpTooltip :label="copy.appProxyManualTitle" :text="manualHelp" pinnable /></div>
            <div v-if="protocols.length > 1" class="app-proxy-protocols" role="group" :aria-label="copy.localProtocol"><button v-for="value in protocols" :key="value" type="button" :aria-pressed="protocol === value" @click="protocol = value">{{ value.toUpperCase() }}</button></div>
            <span v-else class="app-proxy-protocol-label">{{ protocol.toUpperCase() }}</span>
          </header>
          <div class="app-proxy-address-row">
            <code>{{ fullAddress }}</code>
            <button class="secondary-action app-proxy-copy-address" type="button" @click="copyField('address', fullAddress)"><svg viewBox="0 0 20 20" aria-hidden="true"><path v-if="copiedField === 'address'" d="m4.5 10.2 3.2 3.2 7.8-7.8"/><template v-else><rect x="6.5" y="6.5" width="9" height="9" rx="1.6"/><path d="M13.5 6.5V5A1.5 1.5 0 0 0 12 3.5H5A1.5 1.5 0 0 0 3.5 5v7A1.5 1.5 0 0 0 5 13.5h1.5"/></template></svg>{{ copiedField === 'address' ? copy.endpointCopied : copy.appProxyCopyAddress }}</button>
          </div>
          <div class="app-proxy-split"><p class="app-proxy-note">{{ copy.appProxySplitFields }}</p>
          <dl class="app-proxy-fields"><div v-for="field in fields" :key="field.key"><dt>{{ field.label }}</dt><dd><code>{{ field.value }}</code><button class="app-proxy-copy" type="button" :aria-label="`${copy.copyEndpoint}: ${field.label}`" :title="copiedField === field.key ? copy.endpointCopied : copy.copyEndpoint" @click="copyField(field.key, field.value)"><svg v-if="copiedField !== field.key" viewBox="0 0 20 20" aria-hidden="true"><rect x="6.5" y="6.5" width="9" height="9" rx="1.6"/><path d="M13.5 6.5V5A1.5 1.5 0 0 0 12 3.5H5A1.5 1.5 0 0 0 3.5 5v7A1.5 1.5 0 0 0 5 13.5h1.5"/></svg><svg v-else viewBox="0 0 20 20" aria-hidden="true"><path d="m4.5 10.2 3.2 3.2 7.8-7.8"/></svg></button></dd></div></dl>
          </div>
          <span class="app-proxy-announcement" role="status">{{ copiedField ? copy.endpointCopied : '' }}</span>
        </section>
        <section class="app-proxy-launch">
          <header class="app-proxy-section-heading"><h2>{{ copy.appProxyNoSettings }}</h2><button class="secondary-action" type="button" :disabled="busy || (!available && !expanded)" :aria-expanded="expanded" aria-controls="app-proxy-picker" @click="togglePicker">{{ expanded ? copy.appProxyHide : copy.appProxyTryLaunch }}</button></header>
          <div v-if="expanded" id="app-proxy-picker">
            <div v-if="choosing" class="application-picker">
            <div class="application-picker-actions">
              <label class="application-search"><svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="8.5" cy="8.5" r="5"/><path d="m12.2 12.2 4.3 4.3"/></svg><input ref="searchInput" v-model="search" type="search" :aria-label="copy.assistantSearch" :placeholder="copy.assistantSearch" :disabled="loadingApps || busy" /></label>
              <button class="secondary-action" type="button" :disabled="loadingApps || busy" @click="loadApplications">{{ copy.assistantRefreshApps }}</button>
              <button class="secondary-action" type="button" :disabled="busy" @click="browseApplication">{{ copy.assistantBrowse }}</button>
            </div>
            <p v-if="loadingApps" class="app-proxy-note" role="status">{{ copy.assistantLoadingApps }}</p>
            <div v-else-if="filteredApplications.length" class="application-list"><button v-for="app in filteredApplications" :key="app.pid" type="button" :disabled="busy || !app.applicationId || !app.executablePath" :aria-pressed="selected?.id === app.applicationId" @click="chooseRunning(app)"><span class="application-glyph" aria-hidden="true">{{ app.displayName.slice(0, 1).toUpperCase() }}</span><span><strong>{{ app.displayName }}</strong><small :title="app.executablePath || undefined">{{ app.executablePath ? withoutWindowsExtendedPathPrefix(app.executablePath) : copy.assistantPathUnavailable }}</small></span><svg viewBox="0 0 20 20" aria-hidden="true"><path v-if="selected?.id === app.applicationId" d="m4.5 10.2 3.2 3.2 7.8-7.8"/><path v-else d="m7.5 5 5 5-5 5"/></svg></button></div>
            <p v-else-if="!error" class="app-proxy-note">{{ applications.length ? copy.assistantSearchEmpty : copy.assistantNoApps }}</p>
            <div class="app-proxy-picker-help"><HelpTooltip :label="copy.assistantListNoticeTitle" :text="copy.assistantListHint" pinnable /><span>{{ copy.assistantListNoticeTitle }}</span></div>
            </div>
            <div v-if="selected && !choosing" class="app-proxy-selected"><strong :title="withoutWindowsExtendedPathPrefix(selected.executablePath)">{{ selected.displayName }}</strong><button class="app-proxy-text-action" type="button" :disabled="busy" @click="openChoices">{{ copy.appProxyChangeApp }}</button><button ref="launchButton" class="primary-action" type="button" :disabled="busy || !available" :aria-busy="busy" @click="launch">{{ copy.appProxyLaunch }}</button></div>
            <p class="app-proxy-note">{{ copy.appProxyLaunchHint }} {{ selected ? copy.appProxyRunningHint : '' }}</p>
            <p v-if="launchedPid" class="app-proxy-result" role="status"><strong>{{ copy.appProxyLaunched }}</strong>{{ copy.appProxyVerify.replace('{pid}', String(launchedPid)) }}</p>
          </div>
        </section>
      </div>
    </section>
  </main>
</template>

<style src="../application-proxy.css"></style>
