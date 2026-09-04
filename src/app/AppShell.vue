<script setup lang="ts">
import { BundleType, getBundleType, getVersion } from "@tauri-apps/api/app";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import ProxyPage from "../features/proxy/components/ProxyPage.vue";
import ApplicationAssistantPage from "../features/application-assistant/components/ApplicationAssistantPage.vue";
import SettingsPage, { type SettingsTab } from "../features/settings/components/SettingsPage.vue";
import {
  compareVersions,
  isOfficialReleaseUrl,
  parseReleaseNotes,
  type GitHubRelease,
  type ReleaseNoteLine,
  type UpdateState
} from "../features/settings/update";
import { backend } from "../shared/api/backend";
import { messages, resolveLocale } from "../shared/i18n";
import type { AppSettings, EnvironmentStatus, ManagedProxyVariable, ProxyCandidate, ProxyEndpoint, TunObservation } from "../shared/types";
import { copyText } from "../shared/utils/clipboard";
import AppHeader from "./components/AppHeader.vue";
import DiagnosticReportDialog from "../features/diagnostic-report/components/DiagnosticReportDialog.vue";

const reportDialog = ref<InstanceType<typeof DiagnosticReportDialog>>();
const reportApplicationId = ref<string>();

const defaultSettings: AppSettings = {
  language: "system",
  theme: "system",
  launchAtStartup: false,
  silentStart: false,
  closeToTray: true,
  proxyVariables: ["http", "https"]
};

const view = ref<"home" | "assistant" | "settings">("home");
const settingsTab = ref<SettingsTab>("general");
const loading = ref(true);
const toggling = ref(false);
const error = ref("");
const settingsError = ref("");
const settingsLoadError = ref("");
const copiedEndpoint = ref(false);
const instanceNoticeVisible = ref(false);
const appVersion = ref("0.1.3");
const latestVersion = ref("");
const updateState = ref<UpdateState>("idle");
const releaseUrl = ref("");
const releasePublishedAt = ref("");
const releaseNotesBody = ref("");
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
const draftSettings = ref<AppSettings>({ ...defaultSettings });
const maximized = ref(false);
const systemDark = window.matchMedia("(prefers-color-scheme: dark)");
const reviewPreview = import.meta.env.DEV && new URLSearchParams(window.location.search).has("impeccable-review");
const appWindow = reviewPreview ? undefined : getCurrentWindow();
let refreshTimer: number | undefined;
let copyTimer: number | undefined;
let instanceNoticeTimer: number | undefined;
let refreshPending = false;
let settingsSaveTimer: number | undefined;
let settingsReady = false;
let saveInFlight = false;
let pendingSettings: AppSettings | undefined;
let pendingUpdate: Awaited<ReturnType<typeof check>> = null;
let unlistenResize: UnlistenFn | undefined;
let unlisten: UnlistenFn[] = [];

const locale = computed(() => resolveLocale(draftSettings.value.language));
const copy = computed(() => messages[locale.value]);
const activeProxyContext = computed(() => environment.value.activeProxy);
const detected = computed(() => activeProxyContext.value.candidate ?? undefined);
const systemProxy = computed(() => candidates.value.find((candidate) => candidate.source.includes("windowsSystemProxy")));
const endpoint = computed(() => detected.value ? `${detected.value.host}:${detected.value.port}` : "");
const updateMessage = computed(() => {
  if (updateState.value === "checking") return copy.value.checkingUpdates;
  if (updateState.value === "latest") return copy.value.latestVersion;
  if (updateState.value === "available") return copy.value.updateAvailable.replace("{version}", latestVersion.value);
  if (updateState.value === "manual") return (manualUpdateReason.value === "bundle"
    ? copy.value.automaticUpdateUnsupported
    : copy.value.signedUpdateUnavailable).replace("{version}", latestVersion.value);
  if (updateState.value === "downloading") return updateProgress.value === null
    ? copy.value.downloadingUpdate
    : copy.value.downloadingUpdateProgress.replace("{progress}", String(updateProgress.value));
  if (updateState.value === "installing") return copy.value.installingUpdate;
  if (updateState.value === "unpublished") return copy.value.noPublishedRelease;
  if (updateState.value === "error") return copy.value.updateCheckFailed;
  return copy.value.notChecked;
});
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
  const theme = draftSettings.value.theme === "system"
    ? (systemDark.matches ? "dark" : "light")
    : draftSettings.value.theme;
  document.documentElement.dataset.theme = theme;
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
  if (refreshPending) return;
  refreshPending = true;
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
    acceptEnvironmentStatus(status);
    tun.value = tunObservation;
  } catch (cause) {
    if (!silent) error.value = String(cause);
  } finally {
    refreshPending = false;
    if (!silent) loading.value = false;
  }
}

function acceptEnvironmentStatus(status: EnvironmentStatus) {
  if (status.activeProxy.revision < activeProxyContext.value.revision) return;
  candidates.value = status.candidates;
  environment.value = status;
}

async function selectActiveProxy(candidateId: string) {
  if (toggling.value) return;
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
    toggling.value = false;
  }
}

async function applyDetectedProxy() {
  toggling.value = true;
  error.value = "";
  try {
    if (!activeProxyContext.value.available) throw new Error(copy.value.activeProxyUnavailable);
    acceptEnvironmentStatus(await backend.syncProxyEnvironment(activeProxyContext.value.revision));
    await refresh(true);
  } catch (cause) {
    error.value = String(cause);
  } finally {
    toggling.value = false;
  }
}


async function applyManualProxy(endpoint: ProxyEndpoint) {
  toggling.value = true;
  error.value = "";
  try {
    acceptEnvironmentStatus(await backend.syncManualProxyEnvironment(endpoint));
    await refresh(true);
  } catch (cause) {
    error.value = String(cause);
  } finally {
    toggling.value = false;
  }
}

function inspectManualEndpoint(endpoint: ProxyEndpoint) {
  return backend.inspectProxyEndpoint(endpoint);
}

async function disableEnvironment() {
  toggling.value = true;
  error.value = "";
  try {
    await backend.disableProxyEnvironment();
    await refresh(true);
  } catch (cause) {
    error.value = String(cause);
  } finally {
    toggling.value = false;
  }
}

async function restoreEnvironment() {
  toggling.value = true;
  error.value = "";
  try {
    await backend.restoreProxyEnvironment();
    await refresh(true);
  } catch (cause) {
    error.value = String(cause);
  } finally {
    toggling.value = false;
  }
}

function openSettings() {
  view.value = "settings";
}

function closeSettings() {
  view.value = "home";
}

function openAssistant() {
  view.value = "assistant";
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
  if (["checking", "downloading", "installing"].includes(updateState.value)) return;
  updateState.value = "checking";
  releaseActionError.value = "";
  updateProgress.value = null;
  pendingUpdate = null;
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), 10_000);
  try {
    const releaseRequest = fetch("https://api.github.com/repos/GKNEETIEMAY/ProxyEnv/releases/latest", {
        cache: "no-store",
        headers: { Accept: "application/vnd.github+json" },
        signal: controller.signal
      })
      .then(async (response): Promise<GitHubRelease | null> => {
        if (response.status === 404) return null;
        if (!response.ok) throw new Error(`GitHub ${response.status}`);
        return await response.json() as GitHubRelease;
      });
    const [releaseResult, updaterResult] = await Promise.allSettled([
      releaseRequest,
      check({ timeout: 10_000 })
    ]);

    const release = releaseResult.status === "fulfilled" ? releaseResult.value : null;
    const signedUpdate = updaterResult.status === "fulfilled" ? updaterResult.value : null;
    if (release) {
      if (!release.tag_name) throw new Error("missing release tag");
      if (!release.html_url || !isOfficialReleaseUrl(release.html_url)) throw new Error("invalid release URL");
      latestVersion.value = release.tag_name.replace(/^v/i, "");
      releaseUrl.value = release.html_url;
      releasePublishedAt.value = release.published_at ?? "";
      releaseNotesBody.value = release.body ?? "";
    } else if (signedUpdate) {
      latestVersion.value = signedUpdate.version;
      releaseUrl.value = `https://github.com/GKNEETIEMAY/ProxyEnv/releases/tag/v${signedUpdate.version}`;
      releasePublishedAt.value = signedUpdate.date ?? "";
      releaseNotesBody.value = signedUpdate.body ?? "";
    } else if (releaseResult.status === "fulfilled") {
      updateState.value = "unpublished";
      latestVersion.value = "";
      releaseUrl.value = "";
      releasePublishedAt.value = "";
      releaseNotesBody.value = "";
      return;
    } else {
      throw new Error("release metadata is unavailable");
    }

    if (signedUpdate && automaticUpdateSupported.value) {
      pendingUpdate = signedUpdate;
      latestVersion.value = signedUpdate.version;
      updateState.value = "available";
      return;
    }
    manualUpdateReason.value = signedUpdate ? "bundle" : "missing";
    updateState.value = compareVersions(latestVersion.value, appVersion.value) > 0
      ? "manual"
      : "latest";
  } catch {
    updateState.value = "error";
  } finally {
    window.clearTimeout(timeout);
  }
}

async function installPendingUpdate() {
  if (!pendingUpdate || updateState.value !== "available") {
    releaseActionError.value = copy.value.updateInstallUnavailable;
    return;
  }
  releaseActionError.value = "";
  updateProgress.value = null;
  updateState.value = "downloading";
  let downloaded = 0;
  let contentLength = 0;
  try {
    await pendingUpdate.downloadAndInstall((event) => {
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
        updateState.value = "installing";
      }
    });
    updateState.value = "installing";
    await relaunch();
  } catch {
    updateState.value = "available";
    updateProgress.value = null;
    releaseActionError.value = copy.value.updateInstallFailed;
  }
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
  if (event.key !== "Escape" || view.value === "home" || event.defaultPrevented) return;
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
  if (saveInFlight || !pendingSettings) return;
  const settings = pendingSettings;
  pendingSettings = undefined;
  saveInFlight = true;
  settingsError.value = "";
  try {
    await backend.saveAppSettings(settings);
    settingsLoadError.value = "";
  } catch (cause) {
    if (view.value === "settings") settingsError.value = String(cause);
    else error.value = String(cause);
  } finally {
    saveInFlight = false;
    if (pendingSettings) void flushSettings();
  }
}

function onSystemThemeChange() {
  if (draftSettings.value.theme === "system") applyPresentation();
}

function showSecondInstanceNotice() {
  instanceNoticeVisible.value = true;
  if (instanceNoticeTimer !== undefined) window.clearTimeout(instanceNoticeTimer);
  instanceNoticeTimer = window.setTimeout(() => {
    instanceNoticeVisible.value = false;
  }, 3200);
}

watch([draftSettings, locale], applyPresentation, { deep: true, immediate: true });
watch(draftSettings, () => {
  if (!settingsReady || reviewPreview) return;
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
  try {
    automaticUpdateSupported.value = await getBundleType() === BundleType.Nsis;
  } catch {
    automaticUpdateSupported.value = false;
  }
  try {
    appVersion.value = await getVersion();
  } catch {
    appVersion.value = "0.1.3";
  }
  if (reviewPreview) {
    const preview = new URLSearchParams(window.location.search).get("impeccable-review");
    draftSettings.value = copySettings({ ...defaultSettings, language: "zh-CN" });
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
    if (preview === "settings" || preview === "about") {
      view.value = "settings";
      settingsTab.value = preview === "about" ? "about" : "general";
    } else if (preview === "assistant" || preview === "assistant-result") {
      view.value = "assistant";
    }
    loading.value = false;
    return;
  }
  try {
    const settings = await backend.appSettings();
    draftSettings.value = copySettings(settings);
  } catch (cause) {
    settingsLoadError.value = String(cause);
  }
  settingsReady = true;
  maximized.value = await appWindow!.isMaximized();
  document.documentElement.classList.toggle("window-maximized", maximized.value);
  unlistenResize = await appWindow!.onResized(async () => {
    maximized.value = await appWindow!.isMaximized();
    document.documentElement.classList.toggle("window-maximized", maximized.value);
  });
  systemDark.addEventListener("change", onSystemThemeChange);
  unlisten = await Promise.all([
    listen<EnvironmentStatus>("proxy-state-changed", ({ payload }) => { acceptEnvironmentStatus(payload); }),
    listen<string>("operation-error", ({ payload }) => { error.value = payload; }),
    listen("second-instance-opened", showSecondInstanceNotice)
  ]);
  await refresh();
  refreshTimer = window.setInterval(() => void refresh(true), 5000);
});

onBeforeUnmount(() => {
  if (refreshTimer !== undefined) window.clearInterval(refreshTimer);
  if (copyTimer !== undefined) window.clearTimeout(copyTimer);
  if (instanceNoticeTimer !== undefined) window.clearTimeout(instanceNoticeTimer);
  if (settingsSaveTimer !== undefined) window.clearTimeout(settingsSaveTimer);
  unlistenResize?.();
  window.removeEventListener("keydown", onViewShortcut);
  systemDark.removeEventListener("change", onSystemThemeChange);
  unlisten.forEach((dispose) => dispose());
});
</script>

<template>
  <div class="app-frame" :class="{ maximized }">
    <Transition name="instance-toast">
      <div v-if="instanceNoticeVisible" class="instance-toast" role="status" aria-live="polite">
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m4.5 10.2 3.2 3.2 7.8-7.8" /></svg>
        <span>{{ copy.secondInstanceOpened }}</span>
      </div>
    </Transition>
    <AppHeader
      :copy="copy"
      :maximized="maximized"
      :view="view"
      @close-settings="closeSettings"
      @open-settings="openSettings"
      @open-report="reportDialog?.open()"
      @minimize="minimizeWindow"
      @toggle-maximize="toggleMaximizeWindow"
      @close="closeWindow"
    />

    <div v-if="activeProxyContext.candidate && !activeProxyContext.available" class="notice notice-warning active-proxy-alert" role="status">
      <p><strong>{{ copy.activeProxyUnavailableTitle }}</strong><span>{{ copy.activeProxyUnavailable }}</span></p>
      <button v-if="view !== 'home'" class="secondary-action" type="button" @click="view = 'home'">{{ copy.selectActiveProxy }}</button>
    </div>

    <Transition name="view-fade" mode="out-in">
    <ProxyPage
      v-if="view === 'home'"
      key="home"
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

    <ApplicationAssistantPage
      v-else-if="view === 'assistant'"
      key="assistant"
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
      v-else-if="view === 'settings'"
      key="settings"
      v-model:settings="draftSettings"
      v-model:tab="settingsTab"
      :copy="copy"
      :settings-error="settingsError"
      :settings-load-error="settingsLoadError"
      :app-version="appVersion"
      :update-state="updateState"
      :update-message="updateMessage"
      :release-version="releaseVersion"
      :release-published-label="releasePublishedLabel"
      :release-notes="displayReleaseNotes"
      :release-url="releaseUrl"
      :release-action-error="releaseActionError"
      :update-progress="updateProgress"
      @check-for-updates="checkForUpdates"
      @install-update="installPendingUpdate"
      @open-release="openLatestRelease"
    />
    </Transition>
    <DiagnosticReportDialog ref="reportDialog" :copy="copy" :locale="locale" :application-id="reportApplicationId" :review-preview="reviewPreview" />
  </div>
</template>
