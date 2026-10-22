<script setup lang="ts">
import { BundleType, getBundleType, getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { check } from "@tauri-apps/plugin-updater";
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, shallowRef, watch } from "vue";
import { ConnectivitySession, connectivityTtlMs, type ConnectivityView } from "../features/proxy/connectivity";
import { RefreshEpoch } from "../features/proxy/refresh-epoch";
import ProxyPage from "../features/proxy/components/ProxyPage.vue";
import ApplicationAssistantPage from "../features/application-assistant/components/ApplicationAssistantPage.vue";
import SettingsPage, { type SettingsTab } from "../features/settings/components/SettingsPage.vue";
import {
  isOfficialReleaseUrl,
  parseReleaseNotes,
  type ReleaseNoteLine,
  type UpdateState
} from "../features/settings/update";
import { checkUpdateMetadata, fetchLatestRelease, resolveUpdate, updateFailure, type Availability, type UpdateFailure } from "../features/settings/update-availability";
import { UpdateInstaller } from "../features/settings/update-installer";
import UpdateInstallDialog from "../features/settings/components/UpdateInstallDialog.vue";
import { backend } from "../shared/api/backend";
import { messages, resolveLocale } from "../shared/i18n";
import type { AppSettings, EnvironmentStatus, ManagedProxyVariable, ProxyCandidate, ProxyEndpoint, TunObservation } from "../shared/types";
import { copyText } from "../shared/utils/clipboard";
import AppHeader from "./components/AppHeader.vue";
import { remoteNavigationState, trayNavigationAction } from "./navigation";
import DiagnosticReportDialog from "../features/diagnostic-report/components/DiagnosticReportDialog.vue";
import NotificationCenter from "../shared/components/NotificationCenter.vue";
import { notify, setOptionalNotificationsEnabled, useNotification } from "../shared/state/notifications";
import { applyTheme, readAppearance, readPalette, saveAppearance, savePalette } from "../shared/theme/palette";

import RemoteBridgePage from "../features/remote-bridge/components/RemoteBridgePage.vue";
import { useRemoteBridge, type BridgeSummary } from "../features/remote-bridge/state";
const { summary: remoteBridgeSummary, refresh: refreshRemoteBridge, ccDetection } = useRemoteBridge();

const reportDialog = ref<InstanceType<typeof DiagnosticReportDialog>>();
const remotePage = ref<InstanceType<typeof RemoteBridgePage>>();
const updateDialog = ref<InstanceType<typeof UpdateInstallDialog>>();
const reportApplicationId = ref<string>();

const defaultSettings: AppSettings = {
  language: "system",
  theme: "system",
  launchAtStartup: false,
  silentStart: false,
  closeToTray: true,
  notificationsEnabled: true,
  notificationSound: false,
  proxyVariables: ["http", "https"]
};

const primaryView = ref<"local" | "remote">("local");
const view = ref<"local" | "remote" | "assistant" | "settings">("local");
const settingsTab = ref<SettingsTab>("general");
const loading = ref(true);
const toggling = ref(false);
const error = ref("");
const settingsError = ref("");
const settingsLoadError = ref("");
const desktopAlerts = ref(false);
const desktopAlertsReady = ref(false);
const desktopAlertsBusy = ref(false);
const desktopAlertsError = ref(false);
const copiedEndpoint = ref(false);
function openBridgeNotice() { openRemote(); void refreshRemoteBridge(); }
const appVersion = ref("0.2.1");
const updateSnapshot = shallowRef<Availability<NonNullable<Awaited<ReturnType<typeof check>>>> | null>(null);
const latestVersion = computed(() => updateSnapshot.value?.version ?? "");
const updateState = ref<UpdateState>("idle");
const releaseUrl = computed(() => updateSnapshot.value?.url ?? "");
const releasePublishedAt = computed(() => updateSnapshot.value?.date ?? "");
const releaseNotesBody = computed(() => updateSnapshot.value?.body ?? "");
const checkFailure = ref<UpdateFailure>("service");
const installStatusError = ref<"updateGuardUnavailable" | "updateBusy" | "updateStateChanged" | "updateInstallError" | null>(null);
const updateAbandoned = ref(false);
const installPreparing = ref(false);
const updateExclusive = ref(false);
const releaseActionError = ref("");
const updateProgress = ref<number | null>(null);
const automaticUpdateSupported = ref(false);
const manualUpdateReason = ref<"bundle" | "missing">("missing");
const environment = ref<EnvironmentStatus>({
  state: "disabled",
  entries: [],
  selectedVariables: [...defaultSettings.proxyVariables],
  activeProxy: { selectedCandidateId: null, candidate: null, selectionSource: "auto", available: false, revision: 0 },
  candidates: [],
  matchesActiveProxy: false,
  snapshotAvailable: false
});
const candidates = ref<ProxyCandidate[]>([]);
const tun = ref<TunObservation>({ state: "unknown", evidence: [] });
const draftSettings = ref<AppSettings>({ ...defaultSettings, theme: readAppearance() });
const palette = ref(readPalette());
const maximized = ref(false);
const systemDark = window.matchMedia("(prefers-color-scheme: dark)");
const reviewPreview = import.meta.env.DEV && new URLSearchParams(window.location.search).has("impeccable-review");
const appWindow = reviewPreview ? undefined : getCurrentWindow();
let refreshTimer: number | undefined;
let copyTimer: number | undefined;
let refreshPending = false;
let refreshAgain = false;
const refreshEpoch = new RefreshEpoch();
const localStale = ref(false);
const localNow = ref(Date.now());
const connectivity = reactive<ConnectivityView>({ result: null, testing: false, cached: false, failed: false });
let connectivityExpiryTimer: number | undefined;
let settingsInFlight: Promise<AppSettings> | undefined;
let settingsSaveError: unknown;
let settingsSaveTimer: number | undefined;
let settingsReady = false;
let saveInFlight = false;
let pendingSettings: AppSettings | undefined;
let savedNotificationSettings = { notificationsEnabled: true, notificationSound: false };
let restoringNotificationSettings = false;
let versionVerified = false;
const installer = new UpdateInstaller({
  prepare: confirmed => invoke<number>("prepare_update_install", { confirmed }),
  validate: lease => invoke<void>("validate_update_install", { lease }),
  release: lease => invoke<void>("release_update_install", { lease }),
});
let unlistenResize: UnlistenFn | undefined;
let unlisten: UnlistenFn[] = [];

const locale = computed(() => resolveLocale(draftSettings.value.language));
const copy = computed(() => messages[locale.value]);
const remoteNavState = computed(() => remoteNavigationState(remoteBridgeSummary.value));
useNotification(error, "error", "app-operation");
useNotification(settingsError, "error", "settings-save");
useNotification(settingsLoadError, "error", "settings-load");
useNotification(releaseActionError, "error", "release-action");
const activeProxyContext = computed(() => environment.value.activeProxy);
const detected = computed(() => activeProxyContext.value.candidate ?? undefined);
const connectivitySession = new ConnectivitySession(connectivity, () => activeProxyContext.value, backend, cause => { error.value = String(cause); });
watch([() => activeProxyContext.value.revision, () => activeProxyContext.value.available, () => activeProxyContext.value.selectedCandidateId], () => {
  connectivitySession.invalidate();
  if (!reviewPreview) void connectivitySession.readCache();
}, { flush: "sync" });
watch(() => connectivity.result, result => {
  localNow.value = Date.now();
  if (connectivityExpiryTimer !== undefined) window.clearTimeout(connectivityExpiryTimer);
  if (result) {
    const remaining = Date.parse(result.testedAt) + connectivityTtlMs - Date.now();
    if (Number.isFinite(remaining) && remaining > 0) connectivityExpiryTimer = window.setTimeout(() => { localNow.value = Date.now(); }, remaining);
  }
});
function refreshLocalClock() { localNow.value = Date.now(); }
window.addEventListener("focus", refreshLocalClock);
const systemProxy = computed(() => candidates.value.find((candidate) => candidate.source.includes("windowsSystemProxy")));
const endpoint = computed(() => detected.value ? `${detected.value.host}:${detected.value.port}` : "");
const updateMessage = computed(() => {
  if (updateAbandoned.value && updateState.value === "downloading") return copy.value.updateCancelling;
  if (updateState.value === "checking") return copy.value.checkingUpdates;
  if (updateState.value === "latest") return copy.value.latestVersion;
  if (updateState.value === "available") return copy.value.updateAvailable.replace("{version}", latestVersion.value);
  if (updateState.value === "manual" && updateSnapshot.value?.failure) return `${copy.value.updateAvailable.replace("{version}", latestVersion.value)} ${updateFailureMessage(updateSnapshot.value.failure)}`;
  if (updateState.value === "manual") return (manualUpdateReason.value === "bundle"
    ? copy.value.automaticUpdateUnsupported
    : copy.value.signedUpdateUnavailable).replace("{version}", latestVersion.value);
  if (updateState.value === "downloading") return updateProgress.value === null
    ? copy.value.downloadingUpdate
    : copy.value.downloadingUpdateProgress.replace("{progress}", String(updateProgress.value));
  if (updateState.value === "installing") return copy.value.installingUpdate;
  if (updateState.value === "unpublished") return copy.value.noPublishedRelease;
  if (updateState.value === "conflict") return copy.value.updateConflict;
  if (updateState.value === "error") return installStatusError.value ? copy.value[installStatusError.value] : updateFailureMessage(checkFailure.value);
  return copy.value.notChecked;
});
const availableUpdateVersion = computed(() => ["available", "manual"].includes(updateState.value) ? latestVersion.value : "");
function recordUpdateDiagnostic(phase: "release" | "metadata" | "download" | "install" | "guard", outcome: string) {
  if (reviewPreview) return;
  // Backend deserialization allows only enum values; raw exceptions can never enter the log.
  void invoke("record_update_diagnostic", { event: { phase, outcome } }).catch(() => {});
}
function updateFailureMessage(failure: UpdateFailure): string {
  return ({ rateLimit: copy.value.updateRateLimit, timeout: copy.value.updateTimeout, network: copy.value.updateNetwork,
    tls: copy.value.updateTls, proxy: copy.value.updateProxy, signature: copy.value.updateSignature,
    invalidResponse: copy.value.updateInvalidResponse, service: copy.value.updateService, development: copy.value.updateDevelopment })[failure];
}
const releaseVersion = computed(() => latestVersion.value || appVersion.value);
const releasePublishedLabel = computed(() => {
  if (!releasePublishedAt.value) return "";
  const date = new Date(releasePublishedAt.value);
  if (Number.isNaN(date.getTime())) return "";
  return new Intl.DateTimeFormat(locale.value, {
    year: "numeric",
    month: "short",
    day: "numeric"
  }).format(date);
});
const displayReleaseNotes = computed<ReleaseNoteLine[]>(() => {
  const localizedNotes = parseReleaseNotes(releaseNotesBody.value, locale.value);
  return localizedNotes.length > 0
    ? localizedNotes
    : [{ kind: "paragraph", text: copy.value.noReleaseNotes }];
});

function applyPresentation() {
  applyTheme(document.documentElement, draftSettings.value.theme, palette.value, systemDark.matches);
  if (!reviewPreview) saveAppearance(draftSettings.value.theme);
  document.documentElement.lang = locale.value;
  document.title = copy.value.appName;
}

function copySettings(settings: AppSettings): AppSettings {
  return { ...settings, proxyVariables: [...settings.proxyVariables] };
}

function managedVariableKey(name: string): ManagedProxyVariable | undefined {
  const normalized = name.toUpperCase();
  if (normalized === "HTTP_PROXY") return "http";
  if (normalized === "HTTPS_PROXY") return "https";
  if (normalized === "ALL_PROXY") return "all";
  return undefined;
}

function toggleManagedVariable(name: string) {
  const key = managedVariableKey(name);
  if (!key) return;
  const selected = draftSettings.value.proxyVariables;
  if (selected.includes(key)) {
    if (selected.length === 1) return;
    draftSettings.value = {
      ...draftSettings.value,
      proxyVariables: selected.filter((item) => item !== key)
    };
  } else {
    draftSettings.value = {
      ...draftSettings.value,
      proxyVariables: [...selected, key]
    };
  }
}

async function refresh(silent = false) {
  localNow.value = Date.now();
  if (refreshPending || toggling.value) { refreshAgain = true; return; }
  refreshPending = true;
  refreshAgain = false;
  const generation = refreshEpoch.capture();
  if (!silent) {
    loading.value = true;
    error.value = "";
  }
  try {
    const [status, tunObservation] = await Promise.all([
      backend.environmentStatus(),
      backend.tunObservation().catch((cause): TunObservation => ({
        state: "unknown",
        evidence: [{ kind: "enumerationUnavailable", detail: String(cause) }]
      }))
    ]);
    if (refreshEpoch.accepts(generation)) {
      acceptEnvironmentStatus(status);
      tun.value = tunObservation;
      localStale.value = false;
    }
  } catch (cause) {
    if (refreshEpoch.accepts(generation)) {
      localStale.value = true;
      if (!silent) error.value = String(cause);
    }
  } finally {
    refreshPending = false;
    if (!silent) loading.value = false;
    if (refreshAgain && !toggling.value) void refresh(true);
  }
}

function acceptEnvironmentStatus(status: EnvironmentStatus) {
  if (status.activeProxy.revision < activeProxyContext.value.revision) return;
  candidates.value = status.candidates;
  environment.value = status;
}

async function selectActiveProxy(candidateId: string) {
  if (toggling.value) return;
  refreshEpoch.invalidate();
  toggling.value = true;
  error.value = "";
  try {
    if (reviewPreview) {
      const candidate = candidates.value.find((candidate) => candidate.id === candidateId);
      if (candidate?.listening) environment.value.activeProxy = {
        candidate, selectedCandidateId: candidate.id, selectionSource: "user", available: true,
        revision: activeProxyContext.value.revision + 1
      };
      // The preview's fixed environment points to v2rayN; production status always comes from Rust.
      environment.value.matchesActiveProxy = candidate?.id === "review-v2rayn";
      environment.value.state = environment.value.matchesActiveProxy ? "enabled" : "mismatch";
    } else {
      acceptEnvironmentStatus(await backend.selectActiveProxy(candidateId));
    }
  } catch (cause) {
    error.value = String(cause);
    await refresh(true);
  } finally {
    refreshEpoch.invalidate();
    toggling.value = false;
    await refresh(true);
  }
}

async function applyDetectedProxy() {
  if (toggling.value) return;
  refreshEpoch.invalidate();
  toggling.value = true;
  error.value = "";
  try {
    if (!activeProxyContext.value.available) throw new Error(copy.value.activeProxyUnavailable);
    await persistProxySettings();
    acceptEnvironmentStatus(await backend.syncProxyEnvironment(activeProxyContext.value.revision));
    await refresh(true);
  } catch (cause) {
    error.value = String(cause);
  } finally {
    refreshEpoch.invalidate();
    toggling.value = false;
    await refresh(true);
  }
}


async function applyManualProxy(endpoint: ProxyEndpoint) {
  if (toggling.value) return;
  refreshEpoch.invalidate();
  toggling.value = true;
  error.value = "";
  try {
    await persistProxySettings();
    acceptEnvironmentStatus(await backend.syncManualProxyEnvironment(endpoint));
    await refresh(true);
  } catch (cause) {
    error.value = String(cause);
  } finally {
    refreshEpoch.invalidate();
    toggling.value = false;
    await refresh(true);
  }
}

function inspectManualEndpoint(endpoint: ProxyEndpoint) {
  return backend.inspectProxyEndpoint(endpoint);
}

async function disableEnvironment() {
  if (toggling.value) return;
  refreshEpoch.invalidate();
  toggling.value = true;
  error.value = "";
  try {
    acceptEnvironmentStatus(await backend.disableProxyEnvironment());
    await refresh(true);
  } catch (cause) {
    error.value = String(cause);
  } finally {
    refreshEpoch.invalidate();
    toggling.value = false;
    await refresh(true);
  }
}

async function restoreEnvironment() {
  if (toggling.value) return;
  refreshEpoch.invalidate();
  toggling.value = true;
  error.value = "";
  try {
    acceptEnvironmentStatus(await backend.restoreProxyEnvironment());
    await refresh(true);
  } catch (cause) {
    error.value = String(cause);
  } finally {
    refreshEpoch.invalidate();
    toggling.value = false;
    await refresh(true);
  }
}

function openSettings() {
  view.value = "settings";
}

function closeSettings() {
  if (updateExclusive.value && primaryView.value === "remote") { openUpdateAbout(); return; }
  view.value = primaryView.value;
}

function openLocal() {
  primaryView.value = "local";
  view.value = "local";
}

function openRemote() {
  if (updateExclusive.value) {
    notify(copy.value.rbUpdateInProgress, "info", "update-bridge-paused");
    openUpdateAbout();
    return;
  }
  primaryView.value = "remote";
  view.value = "remote";
}

function openAssistant() {
  view.value = "assistant";
}

function acceptRemoteBridgeSummary(summary: BridgeSummary) {
  remoteBridgeSummary.value = summary;
}

let trayNavigationRevision = 0;
async function handleTrayNavigation(payload: unknown) {
  const action = trayNavigationAction(payload);
  if (!action) return;
  const revision = ++trayNavigationRevision;
  if (document.querySelector("dialog[open]")) {
    notify(copy.value.rbCapabilityBusy, "info", "tray-busy");
    return;
  }
  if (action === "local") { openLocal(); return; }
  openRemote();
  if (action === "remote" || updateExclusive.value) return;
  await refreshRemoteBridge();
  await nextTick();
  if (revision !== trayNavigationRevision || view.value !== "remote") return;
  if (updateExclusive.value) { openRemote(); return; }
  remotePage.value?.requestTrayCapability(action);
}

async function copyEndpoint(candidate?: ProxyCandidate) {
  const value = candidate ? `${candidate.host}:${candidate.port}` : endpoint.value;
  if (!value) return;
  try {
    await copyText(value);
    copiedEndpoint.value = true;
    if (copyTimer !== undefined) window.clearTimeout(copyTimer);
    copyTimer = window.setTimeout(() => { copiedEndpoint.value = false; }, 1600);
  } catch (cause) {
    error.value = `${copy.value.copyFailed}: ${String(cause)}`;
  }
}

async function checkForUpdates() {
  if (installer.busy || ["checking", "downloading", "installing"].includes(updateState.value)) return;
  if (reviewPreview) {
    // Explicit browser fixture only: no fetch, updater IPC, downloads or installs.
    updateSnapshot.value = { state: "manual", version: "0.2.2", url: "https://github.com/GKNEETIEMAY/ProxyEnv/releases/tag/v0.2.2", body: "", date: "", handle: null, source: "release" };
    updateState.value = "manual";
    return;
  }
  if (import.meta.env.DEV && import.meta.env.VITE_UPDATE_DIAGNOSTICS !== "1") {
    checkFailure.value = "development";
    updateState.value = "error";
    return;
  }
  updateState.value = "checking";
  releaseActionError.value = "";
  updateProgress.value = null;
  const previousHandle = updateSnapshot.value?.handle;
  updateSnapshot.value = null;
  updateAbandoned.value = false;
  installStatusError.value = null;
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), 10_000);
  try {
    await previousHandle?.close();
    const releaseRequest = fetchLatestRelease(fetch, controller.signal);
    const [releaseResult, updaterResult] = await Promise.allSettled([
      releaseRequest,
      checkUpdateMetadata(versionVerified ? appVersion.value : "", releaseRequest, () => check({ timeout: 10_000 }))
    ]);

    const snapshot = resolveUpdate(versionVerified ? appVersion.value : "", releaseResult, updaterResult, automaticUpdateSupported.value);
    recordUpdateDiagnostic("release", releaseResult.status === "fulfilled" ? releaseResult.value ? "success" : "unpublished" : updateFailure(releaseResult.reason));
    recordUpdateDiagnostic("metadata", updaterResult.status === "rejected" ? updateFailure(updaterResult.reason) : snapshot.failure ?? snapshot.state);
    const metadata = updaterResult.status === "fulfilled" ? updaterResult.value : null;
    if (metadata && snapshot.handle !== metadata) await metadata.close();
    updateSnapshot.value = snapshot;
    manualUpdateReason.value = automaticUpdateSupported.value ? "missing" : "bundle";
    checkFailure.value = snapshot.failure ?? "service";
    updateState.value = snapshot.state;
  } catch (cause) {
    checkFailure.value = updateFailure(cause);
    updateState.value = "error";
  } finally {
    window.clearTimeout(timeout);
  }
}

async function installPendingUpdate() {
  if (installer.busy) return;
  if (reviewPreview) { await updateDialog.value?.open(); return; }
  const handle = updateSnapshot.value?.handle;
  if (!handle || updateState.value !== "available") {
    releaseActionError.value = copy.value.updateInstallUnavailable;
    return;
  }
  releaseActionError.value = "";
  updateProgress.value = null;
  installPreparing.value = true;
  let downloaded = 0;
  let contentLength = 0;
  try {
    const outcome = await installer.run(handle, () => updateDialog.value?.open() ?? Promise.resolve(false), phase => {
      updateExclusive.value = true;
      updateState.value = phase === "download" ? "downloading" : "installing";
      recordUpdateDiagnostic(phase, "started");
    }, (event) => {
      if (event.event === "Started") {
        contentLength = event.data.contentLength ?? 0;
        updateProgress.value = contentLength > 0 ? 0 : null;
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        updateProgress.value = contentLength > 0
          ? Math.min(100, Math.round((downloaded / contentLength) * 100))
          : null;
      } else if (event.event === "Finished") {
        updateProgress.value = 100;
      }
    });
    if (outcome === "cancelled") {
      updateState.value = "idle";
      notify(copy.value.updateCancelled, "info", "update-cancel");
    }
    if (outcome !== "deferred") recordUpdateDiagnostic(outcome === "cancelled" ? "guard" : "install", outcome === "cancelled" ? "cancelled" : "success");
  } catch (cause) {
    updateState.value = installer.phase === "prepare" ? "available" : "error";
    checkFailure.value = updateFailure(cause);
    updateProgress.value = null;
    const code = String(cause);
    installStatusError.value = updateAbandoned.value && !installer.guardReleased ? "updateGuardUnavailable"
      : code === "updateBusy" ? "updateBusy" : code === "updateStateChanged" ? "updateStateChanged"
      : installer.phase === "prepare" ? "updateGuardUnavailable"
      : installer.phase === "install" ? "updateInstallError" : null;
    releaseActionError.value = installStatusError.value ? copy.value[installStatusError.value] : updateFailureMessage(checkFailure.value);
    recordUpdateDiagnostic(installer.phase === "prepare" ? "guard" : installer.phase,
      code === "updateBusy" ? "busy" : code === "updateStateChanged" ? "stateChanged" : checkFailure.value);
    if (updateAbandoned.value && installer.guardReleased) {
      updateState.value = "idle";
      releaseActionError.value = "";
      notify(copy.value.updateCancelled, "info", "update-cancel");
    }
  } finally {
    installPreparing.value = false;
    if (installer.guardReleased) updateExclusive.value = false;
    if (installer.phase !== "prepare" && updateSnapshot.value) updateSnapshot.value = { ...updateSnapshot.value, handle: null };
  }
}

async function cancelUpdateInstall() {
  if (installer.phase !== "download" || updateAbandoned.value) return;
  updateAbandoned.value = true;
  try { await installer.cancel(); if (installer.guardReleased) updateExclusive.value = false; }
  catch { releaseActionError.value = copy.value.updateGuardUnavailable; }
}

function openUpdateAbout() {
  openSettings();
  settingsTab.value = "about";
}

async function openLatestRelease() {
  releaseActionError.value = "";
  if (!isOfficialReleaseUrl(releaseUrl.value)) {
    releaseActionError.value = copy.value.releaseOpenFailed;
    return;
  }
  try {
    await openUrl(releaseUrl.value);
  } catch {
    releaseActionError.value = copy.value.releaseOpenFailed;
  }
}

function onViewShortcut(event: KeyboardEvent) {
  // Let the modal own Escape and focus; never navigate the underlying view.
  if (document.querySelector("dialog[open]")) return;
  if (event.key === "," && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    openSettings();
    return;
  }
  if (event.key !== "Escape" || view.value === primaryView.value || event.defaultPrevented) return;
  const target = event.target;
  if (target instanceof HTMLInputElement || target instanceof HTMLSelectElement || target instanceof HTMLTextAreaElement) return;
  event.preventDefault();
  closeSettings();
}

async function minimizeWindow() {
  await appWindow?.minimize();
}

async function toggleMaximizeWindow() {
  await appWindow?.toggleMaximize();
  maximized.value = await appWindow?.isMaximized() ?? false;
  document.documentElement.classList.toggle("window-maximized", maximized.value);
}

async function closeWindow() {
  await appWindow?.close();
}

async function flushSettings() {
  if (saveInFlight) { await settingsInFlight?.catch(() => undefined); return flushSettings(); }
  if (!pendingSettings) return;
  const settings = pendingSettings;
  pendingSettings = undefined;
  saveInFlight = true;
  settingsError.value = "";
  try {
    settingsSaveError = undefined;
    settingsInFlight = backend.saveAppSettings(settings);
    await settingsInFlight;
    savedNotificationSettings = { notificationsEnabled: settings.notificationsEnabled, notificationSound: settings.notificationSound };
    settingsLoadError.value = "";
  } catch (cause) {
    settingsSaveError = cause;
    // Do not claim reminders are off when the durable write failed. Preserve newer edits.
    restoringNotificationSettings = true;
    if (draftSettings.value.notificationsEnabled === settings.notificationsEnabled) draftSettings.value.notificationsEnabled = savedNotificationSettings.notificationsEnabled;
    if (draftSettings.value.notificationSound === settings.notificationSound) draftSettings.value.notificationSound = savedNotificationSettings.notificationSound;
    reconcileQueuedNotifications();
    restoringNotificationSettings = false;
    if (view.value === "settings") settingsError.value = String(cause);
    else error.value = String(cause);
  } finally {
    saveInFlight = false;
    if (pendingSettings) void flushSettings();
  }
}

async function persistProxySettings() {
  if (reviewPreview) return;
  if (settingsSaveTimer !== undefined) window.clearTimeout(settingsSaveTimer);
  pendingSettings = copySettings(draftSettings.value);
  await flushSettings();
  if (settingsSaveError) throw settingsSaveError;
}

function reconcileQueuedNotifications() {
  // Other edits may have queued a save while the previous write was awaiting IPC.
  if (pendingSettings) {
    pendingSettings.notificationsEnabled = draftSettings.value.notificationsEnabled;
    pendingSettings.notificationSound = draftSettings.value.notificationSound;
  }
}

async function loadDesktopAlerts() {
  if (desktopAlertsBusy.value) return;
  desktopAlertsBusy.value = true;
  desktopAlertsError.value = false;
  try {
    desktopAlerts.value = reviewPreview ? false : await backend.desktopAlerts();
    desktopAlertsReady.value = true;
  } catch {
    desktopAlerts.value = false;
    desktopAlertsReady.value = false;
    desktopAlertsError.value = true;
  } finally {
    desktopAlertsBusy.value = false;
  }
}

async function saveDesktopAlerts(enabled: boolean) {
  if (!desktopAlertsReady.value || desktopAlertsBusy.value || !draftSettings.value.notificationsEnabled || settingsLoadError.value) return;
  desktopAlertsBusy.value = true;
  try {
    // Commit only after durable backend consent; a failed save cannot turn the UI on.
    desktopAlerts.value = reviewPreview ? enabled : await backend.setDesktopAlerts(enabled);
  } catch {
    notify(copy.value.desktopAlertsSaveError, "error", "desktop-alerts-save");
  } finally {
    desktopAlertsBusy.value = false;
  }
}

function onSystemThemeChange() {
  if (draftSettings.value.theme === "system") applyPresentation();
}

function showSecondInstanceNotice() {
  notify(copy.value.secondInstanceOpened, "info", "second-instance");
}

watch([draftSettings, locale, palette], applyPresentation, { deep: true, immediate: true });
watch(() => draftSettings.value.notificationsEnabled, setOptionalNotificationsEnabled, { immediate: true });
watch(palette, value => { if (!reviewPreview) savePalette(value); });
watch(draftSettings, () => {
  if (!settingsReady || reviewPreview || restoringNotificationSettings) return;
  if (settingsSaveTimer !== undefined) window.clearTimeout(settingsSaveTimer);
  settingsSaveTimer = window.setTimeout(() => {
    pendingSettings = copySettings(draftSettings.value);
    void flushSettings();
  }, 180);
}, { deep: true, flush: "sync" });
watch([view, settingsTab], ([nextView, nextTab]) => {
  if (reviewPreview || updateState.value !== "idle") return;
  if (nextView === "settings" && nextTab === "about") void checkForUpdates();
});

onMounted(async () => {
  window.addEventListener("keydown", onViewShortcut);
  systemDark.addEventListener("change", onSystemThemeChange);
  try {
    automaticUpdateSupported.value = !import.meta.env.DEV && await getBundleType() === BundleType.Nsis;
  } catch {
    automaticUpdateSupported.value = false;
  }
  try {
    appVersion.value = await getVersion();
    versionVerified = true;
  } catch {
    appVersion.value = "0.2.1";
  }
  if (reviewPreview) {
    const preview = new URLSearchParams(window.location.search).get("impeccable-review");
    draftSettings.value = copySettings({ ...defaultSettings, language: "zh-CN" });
    desktopAlertsReady.value = true;
    environment.value = {
      state: "enabled",
      selectedVariables: ["http", "https"],
      activeProxy: { selectedCandidateId: null, candidate: null, selectionSource: "auto", available: false, revision: 0 },
      candidates: [],
      matchesActiveProxy: true,
      snapshotAvailable: true,
      entries: [
        { name: "HTTP_PROXY", value: "http://127.0.0.1:10809", exists: true },
        { name: "HTTPS_PROXY", value: "http://127.0.0.1:10809", exists: true },
        { name: "ALL_PROXY", value: "socks5://127.0.0.1:10808", exists: true },
        { name: "NO_PROXY", value: "localhost,127.0.0.1,::1", exists: true }
      ]
    };
    const primaryPreviewCandidate: ProxyCandidate = {
      id: "review-v2rayn",
      clientName: "v2rayN",
      iconKey: "v2rayn",
      processName: "v2rayN.exe",
      host: "127.0.0.1",
      port: 10809,
      protocol: "mixed",
      source: ["windowsSystemProxy", "processListener", "protocolProbe"],
      confidence: "veryHigh",
      listening: true
    };
    if (preview === "tun-only") {
      candidates.value = [];
    } else if (preview === "multi-client" || preview === "active-unavailable") {
      candidates.value = [primaryPreviewCandidate, {
        id: "review-clash-verge",
        clientName: "Clash Verge Rev",
        iconKey: "clash-verge-rev",
        processName: "clash-verge.exe",
        host: "127.0.0.1",
        port: 7897,
        protocol: "mixed",
        source: ["processListener", "protocolProbe"],
        confidence: "high",
        listening: true
      }, {
        id: "review-hiddify-stale",
        clientName: "Hiddify",
        iconKey: "hiddify",
        processName: "Hiddify.exe",
        host: "127.0.0.1",
        port: 2334,
        protocol: "socks5",
        source: ["windowsSystemProxy"],
        confidence: "medium",
        listening: false
      }];
    } else {
      candidates.value = [primaryPreviewCandidate];
    }
    if (preview !== "tun-only") environment.value.activeProxy = {
      candidate: primaryPreviewCandidate, selectedCandidateId: primaryPreviewCandidate.id,
      selectionSource: "systemProxy", available: true, revision: 1
    };
    if (preview === "active-unavailable") {
      candidates.value = candidates.value.filter((candidate) => candidate.id !== primaryPreviewCandidate.id);
      environment.value.activeProxy = {
        candidate: { ...primaryPreviewCandidate, listening: false }, selectedCandidateId: primaryPreviewCandidate.id,
        selectionSource: "user", available: false, revision: 2
      };
    }
    tun.value = {
      state: "detected",
      interfaceName: preview === "tun-only" ? "bby104_2" : "singbox_tun",
      description: preview === "tun-only" ? "Unknown virtual network adapter" : "Wintun Userspace Tunnel",
      evidence: [
        { kind: "virtualAdapterName", interfaceName: preview === "tun-only" ? "bby104_2" : "singbox_tun", detail: "recognized virtual adapter" },
        { kind: "broadRoute", interfaceName: preview === "tun-only" ? "bby104_2" : "singbox_tun", detail: "split-default route" }
      ]
    };
    if (preview === "settings" || preview === "about" || preview === "update") {
      view.value = "settings";
      settingsTab.value = preview === "settings" ? "general" : "about";
      if (preview === "update") {
        updateSnapshot.value = { state: "available", version: "0.2.2", url: "https://github.com/GKNEETIEMAY/ProxyEnv/releases/tag/v0.2.2", body: "", date: "", handle: null, source: "release" };
        updateState.value = "available";
      }
    } else if (preview === "assistant" || preview === "assistant-result") {
      view.value = "assistant";
    } else if (["remote", "remote-connected", "remote-connected-advanced", "remote-connected-tray", "remote-auth", "remote-auth-rejected", "remote-auth-completing", "remote-auth-timeout", "remote-auth-unavailable"].includes(preview ?? "")) {
      openRemote();
      if (["remote-connected", "remote-connected-advanced", "remote-connected-tray"].includes(preview ?? "")) remoteBridgeSummary.value = {
        status: "connected",
        target: { id: "openssh|preview|aliyun-dev", displayName: "aliyun-dev", source: "openssh", sourceLabel: "OpenSSH", configPath: "~\\.ssh\\config", sshAlias: "aliyun-dev", host: "8.138.152.49", user: "lxl", port: 22, identityFile: null, authenticationMethod: "password", available: true, compatibility: "compatible", unavailableReason: null, canOpenVscode: true, canOpenMobaxterm: false },
        proxy: { local: { host: "127.0.0.1", port: 10809, protocol: "mixed" }, remotePort: 23841 },
        cc: { local: { host: "127.0.0.1", port: 15721, protocol: "http" }, remotePort: 31472 },
        proxyStatus: "connected",
        ccStatus: "connected",
        activeProxyRevision: 1,
        environment: "export HTTP_PROXY=http://127.0.0.1:23841\nexport HTTPS_PROXY=http://127.0.0.1:23841\nexport ALL_PROXY=socks5h://127.0.0.1:23841",
        codexConfigured: true,
        claudeConfigured: false,
        codexExtension: "notConfigured",
        claudeExtension: "notConfigured",
        error: null,
        sshAuth: { mode: "nonInteractive", method: "identityFile", authenticated: true, passwordStored: false }
      };
    }
    loading.value = false;
    if (preview === "remote-connected-tray") {
      await nextTick();
      remotePage.value?.requestTrayCapability("proxy");
    }
    return;
  }
  try {
    const settings = await backend.appSettings();
    draftSettings.value = copySettings(settings);
    savedNotificationSettings = { notificationsEnabled: settings.notificationsEnabled, notificationSound: settings.notificationSound };
  } catch (cause) {
    settingsLoadError.value = String(cause);
  }
  settingsReady = true;
  void loadDesktopAlerts();
  // One non-blocking check per launch; window visibility and tab switches do not restart it.
  if (!import.meta.env.DEV) void checkForUpdates();
  maximized.value = await appWindow!.isMaximized();
  document.documentElement.classList.toggle("window-maximized", maximized.value);
  unlistenResize = await appWindow!.onResized(async () => {
    maximized.value = await appWindow!.isMaximized();
    document.documentElement.classList.toggle("window-maximized", maximized.value);
  });
  unlisten = await Promise.all([
    listen<EnvironmentStatus>("proxy-state-changed", ({ payload }) => { refreshEpoch.invalidate(); acceptEnvironmentStatus(payload); }),
    listen<string>("operation-error", ({ payload }) => { error.value = payload; }),
    listen("second-instance-opened", showSecondInstanceNotice),
    listen("bridge-notification-open", openBridgeNotice),
    listen<unknown>("tray-navigation", ({ payload }) => { void handleTrayNavigation(payload); })
  ]);
  await refresh();
  refreshTimer = window.setInterval(() => void refresh(true), 5000);
});

onBeforeUnmount(() => {
  connectivitySession.invalidate();
  if (connectivityExpiryTimer !== undefined) window.clearTimeout(connectivityExpiryTimer);
  window.removeEventListener("focus", refreshLocalClock);
  if (refreshTimer !== undefined) window.clearInterval(refreshTimer);
  if (copyTimer !== undefined) window.clearTimeout(copyTimer);
  if (settingsSaveTimer !== undefined) window.clearTimeout(settingsSaveTimer);
  unlistenResize?.();
  window.removeEventListener("keydown", onViewShortcut);
  systemDark.removeEventListener("change", onSystemThemeChange);
  unlisten.forEach((dispose) => dispose());
});
</script>

<template>
  <div class="app-frame" :class="{ maximized }">
    <NotificationCenter :close-label="copy.rbNotificationClose" />
    <AppHeader
      :copy="copy"
      :maximized="maximized"
      :view="view"
      :remote-nav-state="remoteNavState"
      :update-version="availableUpdateVersion"
      @close-settings="closeSettings"
      @open-local="openLocal"
      @open-remote="openRemote"
      @open-settings="openSettings"
      @open-update="openUpdateAbout"
      @open-report="reportDialog?.open()"
      @minimize="minimizeWindow"
      @toggle-maximize="toggleMaximizeWindow"
      @close="closeWindow"
    />

    <div v-if="activeProxyContext.candidate && !activeProxyContext.available" class="notice notice-warning active-proxy-alert" role="status">
      <p><strong>{{ copy.activeProxyUnavailableTitle }}</strong><span>{{ copy.activeProxyUnavailable }}</span></p>
      <button v-if="view !== 'local'" class="secondary-action" type="button" @click="openLocal">{{ copy.selectActiveProxy }}</button>
    </div>

    <div class="view-stage">
    <div class="view-outlet">
    <div v-show="view === 'local'" class="view-pane local-view-pane">
    <ProxyPage
      :copy="copy"
      :environment="environment"
      :candidates="candidates"
      :detected="detected"
      :system-proxy="systemProxy"
      :tun="tun"
      :error="error"
      :loading="loading"
      :toggling="toggling"
      :copied-endpoint="copiedEndpoint"
      :selected-variables="draftSettings.proxyVariables"
      :inspect-manual-endpoint="inspectManualEndpoint"
      :stale="localStale"
      :connectivity="connectivity"
      :now="localNow"
      @test-connectivity="connectivitySession.test()"
      @refresh="refresh(false)"
      @apply-detected="applyDetectedProxy"
      @select-active="selectActiveProxy"
      @apply-manual="applyManualProxy"
      @disable="disableEnvironment"
      @restore="restoreEnvironment"
      @copy-endpoint="copyEndpoint"
      @toggle-variable="toggleManagedVariable"
      @open-assistant="openAssistant"
    />
    </div>

    <div v-show="view === 'remote'" class="view-pane remote-view-pane" :inert="updateExclusive" :aria-busy="updateExclusive">
    <RemoteBridgePage
      ref="remotePage"
      :visible="view === 'remote'"
      :copy="copy"
      :active-proxy="activeProxyContext"
      :summary="remoteBridgeSummary"
      :cc-detection="ccDetection"
      :review-preview="reviewPreview"
      :update-locked="updateExclusive"
      @refresh="refreshRemoteBridge"
      @connected="acceptRemoteBridgeSummary"
      @open-report="reportDialog?.open()"
    />
    </div>

    <ApplicationAssistantPage
      v-if="view === 'assistant'"
      :copy="copy"
      :review-preview="reviewPreview"
      :active-proxy-context="activeProxyContext"
      :system-proxy="systemProxy"
      :active-proxy="activeProxyContext.available ? detected : undefined"
      :tun="tun"
      :proxy-available="activeProxyContext.available"
      :network-loading="loading"
      @selection-change="reportApplicationId = $event"
    />

    <SettingsPage
      v-if="view === 'settings'"
      v-model:settings="draftSettings"
      v-model:tab="settingsTab"
      v-model:palette="palette"
      :copy="copy"
      :settings-error="settingsError"
      :settings-load-error="settingsLoadError"
      :desktop-alerts="desktopAlerts"
      :desktop-alerts-ready="desktopAlertsReady"
      :desktop-alerts-busy="desktopAlertsBusy"
      :desktop-alerts-error="desktopAlertsError"
      :app-version="appVersion"
      :update-state="updateState"
      :update-message="updateMessage"
      :update-source="updateSnapshot?.source === 'metadata' ? copy.updateMetadataSource : 'GitHub Releases'"
      :release-version="releaseVersion"
      :release-published-label="releasePublishedLabel"
      :release-notes="displayReleaseNotes"
      :release-url="releaseUrl"
      :release-action-error="releaseActionError"
      :update-progress="updateProgress"
      :install-preparing="installPreparing"
      :update-abandoned="updateAbandoned"
      @check-for-updates="checkForUpdates"
      @install-update="installPendingUpdate"
      @cancel-update="cancelUpdateInstall"
      @open-release="openLatestRelease"
      @desktop-alerts-change="saveDesktopAlerts"
      @retry-desktop-alerts="loadDesktopAlerts"
    />
    </div>
    </div>
    <DiagnosticReportDialog ref="reportDialog" :copy="copy" :locale="locale" :application-id="reportApplicationId" :review-preview="reviewPreview" />
    <UpdateInstallDialog ref="updateDialog" :copy="copy" />
  </div>
</template>
