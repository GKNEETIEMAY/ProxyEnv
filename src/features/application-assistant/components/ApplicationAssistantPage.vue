<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { backend } from "../../../shared/api/backend";
import type { Copy } from "../../../shared/i18n";
import type { ApplicationDiagnosis, ManagedApplication, ProxyCandidate, RuleChangePreview, RunningApplication, TunObservation } from "../../../shared/types";
import NetworkObservationPanel from "../../network-observation/components/NetworkObservationPanel.vue";
import ApplicationProxyGuideDialog from "./ApplicationProxyGuideDialog.vue";

const props = defineProps<{ copy: Copy; reviewPreview?: boolean; systemProxy?: ProxyCandidate; activeProxy?: ProxyCandidate; tun: TunObservation; proxyAvailable: boolean; networkLoading: boolean }>();

type ResultState = { success: boolean; title: string; detail: string; backupId?: string };

const applications = ref<RunningApplication[]>([]);
const selected = ref<ManagedApplication>();
const diagnosis = ref<ApplicationDiagnosis>();
const rulePreview = ref<RuleChangePreview>();
const result = ref<ResultState>();
const busy = ref(false);
const loadingApps = ref(true);
const error = ref("");
const restorePending = ref(false);
const showAdvanced = ref(false);
const proxyGuide = ref<InstanceType<typeof ApplicationProxyGuideDialog>>();
const AUTHORIZATION_REFRESH_INTERVAL_MS = 4 * 60 * 1000;
let authorizationRefreshTimer: ReturnType<typeof setInterval> | undefined;

const diagnosisTitle = computed(() => {
  if (!diagnosis.value) return "";
  return ({
    normal: props.copy.assistantReadyTitle,
    canLaunchWithProxy: props.copy.assistantProxyLaunchTitle,
    knownApplicationRule: props.copy.assistantRuleTitle,
    unsupported: props.copy.assistantUnsupportedTitle
  })[diagnosis.value.summary];
});
const applicationCount = computed(() => props.copy.assistantFoundApps.replace("{count}", String(applications.value.length)));

const diagnosisBody = computed(() => {
  if (!diagnosis.value) return "";
  return ({
    normal: props.copy.assistantReadyBody,
    canLaunchWithProxy: props.copy.assistantProxyLaunchBody,
    knownApplicationRule: props.copy.assistantRuleBody,
    unsupported: props.copy.assistantUnsupportedBody
  })[diagnosis.value.summary];
});

function managedFromRunning(application: RunningApplication): ManagedApplication | undefined {
  if (!application.applicationId || !application.executablePath) return undefined;
  return {
    id: application.applicationId,
    displayName: application.displayName,
    executablePath: application.executablePath,
    iconKey: null,
    ruleId: null,
    lastAction: null
  };
}

function failureDetail(cause: unknown): string {
  const message = String(cause);
  return message.toLocaleLowerCase("en-US").includes("invalid application:")
    ? props.copy.assistantInvalidExecutable
    : message;
}

function failure(cause: unknown) {
  const detail = failureDetail(cause);
  error.value = `${props.copy.assistantErrorWhat}: ${detail} ${props.copy.assistantErrorUnchanged} ${props.copy.assistantErrorNext}`;
}

function isExpiredAuthorization(cause: unknown): boolean {
  return String(cause).includes("application authorization is missing or expired");
}

function isRenewCommandUnavailable(cause: unknown): boolean {
  const message = String(cause).toLocaleLowerCase("en-US");
  return message.includes("command renew_application_authorization not found")
    || message.includes("unknown command") && message.includes("renew_application_authorization");
}

function comparableExecutablePath(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  return /^(?:[a-z]:|\/\/)/i.test(normalized) ? normalized.toLocaleLowerCase("en-US") : normalized;
}

function updateSelectedAuthorization(application: ManagedApplication) {
  selected.value = application;
  if (diagnosis.value) diagnosis.value = { ...diagnosis.value, application };
}

async function recoverRunningApplicationAuthorization(application: ManagedApplication): Promise<ManagedApplication | undefined> {
  const refreshed = await backend.runningApplications();
  applications.value = refreshed;
  const expectedPath = comparableExecutablePath(application.executablePath);
  const match = refreshed.find((candidate) =>
    candidate.executablePath
    && comparableExecutablePath(candidate.executablePath) === expectedPath
  );
  return match ? managedFromRunning(match) : undefined;
}

async function ensureSelectedAuthorization(): Promise<ManagedApplication> {
  const application = selected.value;
  if (!application) throw props.copy.assistantAuthorizationExpired;
  try {
    const renewed = await backend.renewApplicationAuthorization(application.id);
    updateSelectedAuthorization(renewed);
    return renewed;
  } catch (cause) {
    if (isRenewCommandUnavailable(cause)) {
      const recovered = await recoverRunningApplicationAuthorization(application);
      if (recovered) {
        updateSelectedAuthorization(recovered);
        return recovered;
      }
      return application;
    }
    if (!isExpiredAuthorization(cause)) throw cause;
    const recovered = await recoverRunningApplicationAuthorization(application);
    if (!recovered) {
      selected.value = undefined;
      diagnosis.value = undefined;
      rulePreview.value = undefined;
      throw props.copy.assistantAuthorizationExpired;
    }
    updateSelectedAuthorization(recovered);
    return recovered;
  }
}

async function keepSelectedAuthorizationAlive() {
  if (!selected.value || busy.value || props.reviewPreview) return;
  const application = selected.value;
  try {
    const renewed = await backend.renewApplicationAuthorization(application.id);
    if (selected.value?.id === application.id) updateSelectedAuthorization(renewed);
  } catch (cause) {
    if (!isExpiredAuthorization(cause) && !isRenewCommandUnavailable(cause)) return;
    try {
      const recovered = await recoverRunningApplicationAuthorization(application);
      if (recovered && selected.value?.id === application.id) updateSelectedAuthorization(recovered);
    } catch {
      // Recovery is repeated when the user performs the next explicit action.
    }
  }
}

async function loadApplications() {
  loadingApps.value = true;
  error.value = "";
  try {
    applications.value = await backend.runningApplications();
  } catch (cause) {
    failure(cause);
  } finally {
    loadingApps.value = false;
  }
}

async function browseApplication() {
  error.value = "";
  try {
    const application = await backend.pickApplication();
    if (application) await inspectApplication(application);
  } catch (cause) {
    failure(cause);
  }
}

async function inspectApplication(application: ManagedApplication) {
  selected.value = application;
  busy.value = true;
  error.value = "";
  result.value = undefined;
  rulePreview.value = undefined;
  try {
    const authorized = await ensureSelectedAuthorization();
    diagnosis.value = await backend.diagnoseApplication(authorized.id);
  } catch (cause) {
    diagnosis.value = undefined;
    failure(cause);
  } finally {
    busy.value = false;
  }
}

async function prepareRuleFix() {
  if (!selected.value) return;
  busy.value = true;
  error.value = "";
  try {
    const application = await ensureSelectedAuthorization();
    rulePreview.value = await backend.previewApplicationRuleFix(application.id);
    if (rulePreview.value.state !== "ready" || !rulePreview.value.plan) {
      throw new Error(`${props.copy.assistantRuleUnavailable} (${rulePreview.value.state})`);
    }
  } catch (cause) {
    rulePreview.value = undefined;
    failure(cause);
  } finally {
    busy.value = false;
  }
}

async function applyRuleFix() {
  if (!selected.value || !rulePreview.value?.plan) return;
  busy.value = true;
  error.value = "";
  try {
    const application = await ensureSelectedAuthorization();
    const applied = await backend.applyApplicationRuleFix(application.id, rulePreview.value.plan);
    if (applied.state !== "applied") throw new Error(`${props.copy.assistantRuleApplyFailed} (${applied.state})`);
    result.value = {
      success: true,
      title: props.copy.assistantRuleApplied,
      detail: applied.restartRequired ? props.copy.assistantRestartRequired : props.copy.assistantChangeVerified,
      backupId: applied.backup?.id
    };
  } catch (cause) {
    failure(cause);
  } finally {
    busy.value = false;
  }
}

async function launchSelectedApplication(mode: "proxy" | "direct", trackGlobalBusy = true) {
  if (!selected.value) throw props.copy.assistantAuthorizationExpired;
  if (trackGlobalBusy) busy.value = true;
  error.value = "";
  try {
    const application = await ensureSelectedAuthorization();
    return mode === "proxy"
      ? await backend.launchApplicationWithProxy(application.id)
      : await backend.launchApplicationWithoutProxy(application.id);
  } finally {
    if (trackGlobalBusy) busy.value = false;
  }
}

async function launch(mode: "proxy" | "direct") {
  try {
    const backupId = result.value?.backupId;
    const launched = await launchSelectedApplication(mode);
    result.value = {
      success: true,
      title: mode === "proxy" ? props.copy.assistantLaunchedWithProxy : props.copy.assistantLaunchedDirect,
      detail: props.copy.assistantLaunchResult.replace("{pid}", String(launched.pid)),
      backupId
    };
  } catch (cause) {
    failure(cause);
  }
}

async function launchManualProxyProcess(): Promise<string> {
  if (props.reviewPreview) {
    return props.copy.assistantLaunchResult.replace("{pid}", "12480");
  }
  try {
    const launched = await launchSelectedApplication("direct", false);
    return props.copy.assistantLaunchResult.replace("{pid}", String(launched.pid));
  } catch (cause) {
    throw `${props.copy.assistantErrorWhat}: ${failureDetail(cause)} ${props.copy.assistantErrorUnchanged} ${props.copy.assistantErrorNext}`;
  }
}

function openProxyGuide() {
  proxyGuide.value?.open();
}

async function restoreRule() {
  if (!result.value?.backupId) return;
  busy.value = true;
  error.value = "";
  try {
    const restored = await backend.restoreApplicationRuleChange(result.value.backupId);
    if (restored.state !== "restored") throw new Error(`${props.copy.assistantRestoreFailed} (${restored.state})`);
    result.value = { success: true, title: props.copy.assistantRestored, detail: props.copy.assistantChangeVerified };
    restorePending.value = false;
  } catch (cause) {
    failure(cause);
  } finally {
    busy.value = false;
  }
}

function startOver() {
  selected.value = undefined;
  diagnosis.value = undefined;
  rulePreview.value = undefined;
  result.value = undefined;
  error.value = "";
  restorePending.value = false;
  showAdvanced.value = false;
  void loadApplications();
}

function configValue(value: unknown): string {
  return typeof value === "string" ? value : JSON.stringify(value);
}

onMounted(async () => {
  if (props.reviewPreview) {
    applications.value = [
      { pid: 8420, applicationId: "review-code", processName: "Code.exe", displayName: "Visual Studio Code", executablePath: "C:\\Program Files\\Microsoft VS Code\\Code.exe", iconAvailable: false },
      { pid: 9132, applicationId: "review-discord", processName: "Discord.exe", displayName: "Discord", executablePath: "C:\\Users\\demo\\AppData\\Local\\Discord\\Discord.exe", iconAvailable: false }
    ];
    loadingApps.value = false;
    selected.value = managedFromRunning(applications.value[0]);
    diagnosis.value = {
      application: selected.value!, proxyAvailable: true, systemProxyEnabled: false,
      proxyEnvironmentState: "enabled", tunObservation: "possible", knownRule: undefined,
      proxyConnectivityState: "reachable", applicationNetworkState: "proxyLaunchRecommended",
      recommendedAction: "launchWithProxy", summary: "canLaunchWithProxy"
    };
    if (new URLSearchParams(window.location.search).get("impeccable-review") === "assistant-result") {
      result.value = {
        success: true,
        title: props.copy.assistantLaunchedWithProxy,
        detail: props.copy.assistantLaunchResult.replace("{pid}", "12480")
      };
    }
    return;
  }
  await loadApplications();
  authorizationRefreshTimer = setInterval(() => void keepSelectedAuthorizationAlive(), AUTHORIZATION_REFRESH_INTERVAL_MS);
});

onBeforeUnmount(() => {
  if (authorizationRefreshTimer) clearInterval(authorizationRefreshTimer);
});
</script>

<template>
  <main class="page assistant-page">
    <div class="assistant-intro">
      <div><span class="eyebrow">{{ copy.assistantEyebrow }}</span><h1>{{ copy.assistantTitle }}</h1><p>{{ copy.assistantIntro }}</p></div>
      <ol class="assistant-steps" :aria-label="copy.assistantProgress">
        <li :class="{ active: !diagnosis }">1</li><li :class="{ active: diagnosis && !result }">2</li><li :class="{ active: result }">3</li>
      </ol>
    </div>

    <div v-if="error" class="notice notice-error" role="alert"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 8v5m0 3.5v.01M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" /></svg><p><strong>{{ copy.operationFailed }}</strong><span>{{ error }}</span></p></div>

    <section v-if="!diagnosis && !busy" class="assistant-section application-picker">
      <div class="assistant-section-heading"><div><h2>{{ copy.assistantChooseApp }}</h2><p>{{ copy.assistantChooseHint }}</p></div><div class="application-picker-actions"><span role="status" aria-live="polite">{{ applicationCount }}</span><button class="secondary-action browse-action" type="button" :disabled="loadingApps" @click="loadApplications"><svg :class="{ spinning: loadingApps }" viewBox="0 0 20 20" aria-hidden="true"><path d="M16.5 9.5a6.5 6.5 0 1 0-1.9 4.6M16.5 5v4.5H12" /></svg>{{ copy.assistantRefreshApps }}</button><button class="secondary-action browse-action" type="button" @click="browseApplication"><svg viewBox="0 0 20 20" aria-hidden="true"><path d="M3.5 6.5h5l1.4 1.7h6.6v7.3h-13zM3.5 6.5V4.7h4.2l1.2 1.8" /></svg>{{ copy.assistantBrowse }}</button></div></div>
      <div v-if="loadingApps" class="assistant-loading" role="status">{{ copy.assistantLoadingApps }}</div>
      <div v-else-if="applications.length" class="application-list">
        <button v-for="application in applications" :key="application.pid" type="button" :disabled="!application.applicationId || !application.executablePath" @click="managedFromRunning(application) && inspectApplication(managedFromRunning(application)!)">
          <span class="application-glyph" aria-hidden="true">{{ application.displayName.slice(0, 1).toUpperCase() }}</span>
          <span><strong>{{ application.displayName }}</strong><small>{{ application.executablePath || copy.assistantPathUnavailable }}</small></span>
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m7.5 5 5 5-5 5" /></svg>
        </button>
      </div>
      <div v-else class="assistant-empty"><p>{{ copy.assistantNoApps }}</p><button class="primary-action" type="button" @click="browseApplication">{{ copy.assistantBrowse }}</button></div>
      <div v-if="applications.length" class="notice notice-warning application-list-notice" role="note">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 8.5v4.8m0 3.2v.01M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" /></svg>
        <p><strong>{{ copy.assistantListNoticeTitle }}</strong><span>{{ copy.assistantListHint }}</span></p>
      </div>
      <p class="privacy-note"><svg viewBox="0 0 20 20" aria-hidden="true"><path d="M10 3.5 4.5 5.7v4.1c0 3.3 2.2 5.7 5.5 6.8 3.3-1.1 5.5-3.5 5.5-6.8V5.7z" /></svg>{{ copy.assistantReadOnlyNote }}</p>
    </section>

    <section v-else-if="busy" class="assistant-section assistant-loading-panel" role="status"><span class="assistant-spinner"></span><h2>{{ copy.assistantChecking }}</h2><p>{{ copy.assistantCheckingHint }}</p></section>

    <section v-else-if="result" class="assistant-section result-panel">
      <span class="result-mark" :class="{ success: result.success }" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="m7 12.3 3.2 3.2L17.5 8M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" /></svg></span>
      <h2>{{ result.title }}</h2><p>{{ result.detail }}</p>
      <div v-if="activeProxy" class="result-follow-up">
        <span aria-hidden="true"><svg viewBox="0 0 20 20"><path d="M10 2.8a7.2 7.2 0 1 0 0 14.4 7.2 7.2 0 0 0 0-14.4Zm0 10.8v.1M10 6.2v4.6" /></svg></span>
        <div><strong>{{ copy.assistantProxyStillOfflineTitle }}</strong><p>{{ copy.assistantProxyStillOffline }}</p></div>
      </div>
      <div v-if="result.backupId && !restorePending" class="result-actions"><button class="secondary-action" type="button" @click="restorePending = true">{{ copy.assistantRestore }}</button><button v-if="activeProxy" class="secondary-action" type="button" @click="openProxyGuide">{{ copy.assistantConfigureProxy }}</button><button class="primary-action" type="button" @click="startOver">{{ copy.assistantCheckAnother }}</button></div>
      <div v-else-if="restorePending" class="restore-confirm"><p>{{ copy.assistantRestoreConfirm }}</p><div><button class="secondary-action" type="button" @click="restorePending = false">{{ copy.cancel }}</button><button class="primary-action" type="button" @click="restoreRule">{{ copy.assistantConfirmRestore }}</button></div></div>
      <div v-else class="result-actions"><button v-if="activeProxy" class="secondary-action" type="button" @click="openProxyGuide">{{ copy.assistantConfigureProxy }}</button><button class="primary-action" type="button" @click="startOver">{{ copy.assistantCheckAnother }}</button></div>
    </section>

    <template v-else-if="diagnosis && selected">
      <section class="assistant-section diagnosis-hero">
        <div class="selected-application"><span class="application-glyph" aria-hidden="true">{{ selected.displayName.slice(0, 1).toUpperCase() }}</span><div><span>{{ copy.assistantApplication }}</span><h2>{{ selected.displayName }}</h2><code>{{ selected.executablePath }}</code></div></div>
        <div class="recommendation"><span>{{ copy.assistantRecommendation }}</span><h2>{{ diagnosisTitle }}</h2><p>{{ diagnosisBody }}</p></div>
      </section>

      <NetworkObservationPanel :copy="copy" :system-proxy="systemProxy" :tun="tun" :proxy-available="proxyAvailable" :loading="networkLoading" show-local-proxy context="assistant" />

      <section v-if="rulePreview?.plan" class="assistant-section rule-confirmation">
        <span class="eyebrow">{{ copy.assistantConfirmChange }}</span><h2>{{ copy.assistantRulePreview }}</h2><p>{{ copy.assistantRulePreviewHint }}</p>
        <dl><div><dt>{{ copy.assistantConfigFile }}</dt><dd><code>{{ rulePreview.plan.targetFile }}</code></dd></div><div><dt>{{ copy.assistantConfigField }}</dt><dd><code>{{ rulePreview.plan.fieldPath.join('.') }}</code></dd></div><div><dt>{{ copy.assistantBefore }}</dt><dd><code>{{ configValue(rulePreview.plan.oldValue) }}</code></dd></div><div><dt>{{ copy.assistantAfter }}</dt><dd><code>{{ configValue(rulePreview.plan.newValue) }}</code></dd></div></dl>
        <p class="confirmation-consequence">{{ copy.assistantBackupHint }}</p>
        <div class="assistant-actions"><button class="secondary-action" type="button" @click="rulePreview = undefined">{{ copy.backToEdit }}</button><button class="primary-action" type="button" @click="applyRuleFix">{{ copy.assistantConfirmFix }}</button></div>
      </section>

      <section v-else class="assistant-section assistant-action-panel">
        <div><span class="eyebrow">{{ copy.assistantNextStep }}</span><h2>{{ diagnosis.summary === 'knownApplicationRule' ? copy.assistantOneClickFix : copy.assistantSafeLaunch }}</h2><p>{{ diagnosis.summary === 'knownApplicationRule' ? copy.assistantOneClickFixHint : copy.assistantSafeLaunchHint }}</p></div>
        <div class="assistant-actions">
          <button v-if="diagnosis.summary === 'knownApplicationRule'" class="primary-action" type="button" @click="prepareRuleFix">{{ copy.assistantPreviewFix }}</button>
          <button v-else class="primary-action" type="button" :disabled="!diagnosis.proxyAvailable" @click="launch('proxy')">{{ copy.assistantLaunchWithProxy }}</button>
          <button v-if="activeProxy" class="secondary-action" type="button" @click="openProxyGuide">{{ copy.assistantConfigureProxy }}</button>
          <button class="secondary-action" type="button" @click="launch('direct')">{{ copy.assistantLaunchDirect }}</button>
          <button class="secondary-action" type="button" @click="startOver">{{ copy.assistantChooseAgain }}</button>
        </div>
      </section>

      <details class="assistant-advanced" :open="showAdvanced" @toggle="showAdvanced = ($event.currentTarget as HTMLDetailsElement).open"><summary>{{ copy.assistantAdvanced }}</summary><dl><div><dt>{{ copy.assistantDiagnosisState }}</dt><dd><code>{{ diagnosis.applicationNetworkState }}</code></dd></div><div><dt>{{ copy.assistantEnvironmentState }}</dt><dd><code>{{ diagnosis.proxyEnvironmentState }}</code></dd></div><div v-if="tun.interfaceName"><dt>{{ copy.assistantTunInterface }}</dt><dd><code>{{ tun.interfaceName }}</code></dd></div><div><dt>{{ copy.assistantTunEvidence }}</dt><dd>{{ tun.evidence.length }}</dd></div></dl></details>
    </template>

    <ApplicationProxyGuideDialog ref="proxyGuide" :copy="copy" :candidate="activeProxy" :busy="busy" :launch-process="launchManualProxyProcess" />
  </main>
</template>
