<script setup lang="ts">
import { getVersion } from "@tauri-apps/api/app";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import ProxyPage from "../features/proxy/components/ProxyPage.vue";
import SettingsPage, {
  type SettingsTab,
  type UpdateState
} from "../features/settings/components/SettingsPage.vue";
import { backend } from "../shared/api/backend";
import { messages, resolveLocale } from "../shared/i18n";
import type { AppSettings, EnvironmentStatus, ManagedProxyVariable, ProxyCandidate } from "../shared/types";
import AppHeader from "./components/AppHeader.vue";

const defaultSettings: AppSettings = {
  language: "system",
  theme: "system",
  launchAtStartup: false,
  silentStart: false,
  closeToTray: true,
  proxyVariables: ["http", "https"]
};

const view = ref<"home" | "settings">("home");
const settingsTab = ref<SettingsTab>("general");
const loading = ref(true);
const toggling = ref(false);
const error = ref("");
const settingsError = ref("");
const settingsLoadError = ref("");
const copiedEndpoint = ref(false);
const appVersion = ref("0.1.0");
const latestVersion = ref("");
const updateState = ref<UpdateState>("idle");
const environment = ref<EnvironmentStatus>({ enabled: false, entries: [] });
const candidates = ref<ProxyCandidate[]>([]);
const draftSettings = ref<AppSettings>({ ...defaultSettings });
const maximized = ref(false);
const systemDark = window.matchMedia("(prefers-color-scheme: dark)");
const appWindow = getCurrentWindow();
const reviewPreview = import.meta.env.DEV && new URLSearchParams(window.location.search).has("impeccable-review");
let refreshTimer: number | undefined;
let copyTimer: number | undefined;
let refreshPending = false;
let settingsSaveTimer: number | undefined;
let settingsReady = false;
let saveInFlight = false;
let pendingSettings: AppSettings | undefined;
let persistedProxyVariables: ManagedProxyVariable[] = [...defaultSettings.proxyVariables];
let unlistenResize: UnlistenFn | undefined;
let unlisten: UnlistenFn[] = [];

const locale = computed(() => resolveLocale(draftSettings.value.language));
const copy = computed(() => messages[locale.value]);
const detected = computed(() => candidates.value.find((candidate) => candidate.listening) ?? candidates.value[0]);
const endpoint = computed(() => detected.value ? `${detected.value.host}:${detected.value.port}` : "");
const updateMessage = computed(() => {
  if (updateState.value === "checking") return copy.value.checkingUpdates;
  if (updateState.value === "latest") return copy.value.latestVersion;
  if (updateState.value === "available") return copy.value.updateAvailable.replace("{version}", latestVersion.value);
  if (updateState.value === "unpublished") return copy.value.noPublishedRelease;
  if (updateState.value === "error") return copy.value.updateCheckFailed;
  return copy.value.notChecked;
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
    const [status, detectedCandidates] = await Promise.all([
      backend.environmentStatus(),
      backend.detectProxies()
    ]);
    candidates.value = detectedCandidates;
    const activeProxy = detectedCandidates.find((candidate) => candidate.listening);
    environment.value = status.enabled && activeProxy
      ? await backend.syncProxyEnvironment(activeProxy)
      : status;
  } catch (cause) {
    if (!silent) error.value = String(cause);
  } finally {
    refreshPending = false;
    if (!silent) loading.value = false;
  }
}

async function toggle() {
  toggling.value = true;
  error.value = "";
  try {
    environment.value = environment.value.enabled
      ? await backend.disableProxyEnvironment()
      : await backend.enableProxyEnvironment(detected.value?.listening ? detected.value : undefined);
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

async function copyEndpoint() {
  if (!endpoint.value) return;
  try {
    try {
      await navigator.clipboard.writeText(endpoint.value);
    } catch {
      const fallback = document.createElement("textarea");
      fallback.value = endpoint.value;
      fallback.style.position = "fixed";
      fallback.style.opacity = "0";
      document.body.appendChild(fallback);
      fallback.select();
      const copied = document.execCommand("copy");
      fallback.remove();
      if (!copied) throw new Error("clipboard permission denied");
    }
    copiedEndpoint.value = true;
    if (copyTimer !== undefined) window.clearTimeout(copyTimer);
    copyTimer = window.setTimeout(() => { copiedEndpoint.value = false; }, 1600);
  } catch (cause) {
    error.value = `${copy.value.copyFailed}: ${String(cause)}`;
  }
}

function compareVersions(left: string, right: string): number {
  const normalize = (value: string) => value.replace(/^v/i, "").split(".").map((part) => Number.parseInt(part, 10) || 0);
  const leftParts = normalize(left);
  const rightParts = normalize(right);
  const length = Math.max(leftParts.length, rightParts.length);
  for (let index = 0; index < length; index += 1) {
    const difference = (leftParts[index] ?? 0) - (rightParts[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return 0;
}

async function checkForUpdates() {
  if (updateState.value === "checking") return;
  updateState.value = "checking";
  try {
    const response = await fetch("https://api.github.com/repos/GKNEETIEMAY/ProxyEnv/releases/latest", {
      headers: { Accept: "application/vnd.github+json" }
    });
    if (response.status === 404) {
      updateState.value = "unpublished";
      return;
    }
    if (!response.ok) throw new Error(`GitHub ${response.status}`);
    const release = await response.json() as { tag_name?: string };
    if (!release.tag_name) throw new Error("missing release tag");
    latestVersion.value = release.tag_name.replace(/^v/i, "");
    updateState.value = compareVersions(latestVersion.value, appVersion.value) > 0 ? "available" : "latest";
  } catch {
    updateState.value = "error";
  }
}

function onViewShortcut(event: KeyboardEvent) {
  if (event.key === "," && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    openSettings();
    return;
  }
  if (event.key !== "Escape" || view.value !== "settings" || event.defaultPrevented) return;
  const target = event.target;
  if (target instanceof HTMLInputElement || target instanceof HTMLSelectElement || target instanceof HTMLTextAreaElement) return;
  event.preventDefault();
  closeSettings();
}

async function minimizeWindow() {
  await appWindow.minimize();
}

async function toggleMaximizeWindow() {
  await appWindow.toggleMaximize();
  maximized.value = await appWindow.isMaximized();
  document.documentElement.classList.toggle("window-maximized", maximized.value);
}

async function closeWindow() {
  await appWindow.close();
}

async function flushSettings() {
  if (saveInFlight || !pendingSettings) return;
  const settings = pendingSettings;
  pendingSettings = undefined;
  saveInFlight = true;
  settingsError.value = "";
  try {
    const proxySelectionChanged = settings.proxyVariables.join(",") !== persistedProxyVariables.join(",");
    const saved = await backend.saveAppSettings(settings);
    persistedProxyVariables = [...saved.proxyVariables];
    settingsLoadError.value = "";
    if (proxySelectionChanged && environment.value.enabled) {
      const activeProxy = candidates.value.find((candidate) => candidate.listening);
      if (activeProxy) {
        try {
          environment.value = await backend.syncProxyEnvironment(activeProxy);
        } catch (cause) {
          error.value = String(cause);
        }
      }
    }
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

watch([draftSettings, locale], applyPresentation, { deep: true, immediate: true });
watch(draftSettings, () => {
  if (!settingsReady || reviewPreview) return;
  if (settingsSaveTimer !== undefined) window.clearTimeout(settingsSaveTimer);
  settingsSaveTimer = window.setTimeout(() => {
    pendingSettings = copySettings(draftSettings.value);
    void flushSettings();
  }, 180);
}, { deep: true, flush: "sync" });

onMounted(async () => {
  window.addEventListener("keydown", onViewShortcut);
  try {
    appVersion.value = await getVersion();
  } catch {
    appVersion.value = "0.1.0";
  }
  if (reviewPreview) {
    draftSettings.value = copySettings({ ...defaultSettings, language: "zh-CN" });
    environment.value = {
      enabled: true,
      entries: [
        { name: "HTTP_PROXY", value: "http://127.0.0.1:10809", exists: true },
        { name: "HTTPS_PROXY", value: "http://127.0.0.1:10809", exists: true },
        { name: "ALL_PROXY", value: "socks5://127.0.0.1:10808", exists: true },
        { name: "NO_PROXY", value: "localhost,127.0.0.1,::1", exists: true }
      ]
    };
    candidates.value = [{
      id: "review-v2rayn",
      clientName: "v2rayN",
      iconKey: "v2rayn",
      processName: "v2rayN.exe",
      host: "127.0.0.1",
      port: 10809,
      protocol: "mixed",
      source: ["processListener", "protocolProbe"],
      confidence: "veryHigh",
      listening: true
    }];
    const preview = new URLSearchParams(window.location.search).get("impeccable-review");
    if (preview === "settings" || preview === "about") {
      view.value = "settings";
      settingsTab.value = preview === "about" ? "about" : "general";
    }
    loading.value = false;
    return;
  }
  try {
    const settings = await backend.appSettings();
    draftSettings.value = copySettings(settings);
    persistedProxyVariables = [...settings.proxyVariables];
  } catch (cause) {
    settingsLoadError.value = String(cause);
  }
  settingsReady = true;
  maximized.value = await appWindow.isMaximized();
  document.documentElement.classList.toggle("window-maximized", maximized.value);
  unlistenResize = await appWindow.onResized(async () => {
    maximized.value = await appWindow.isMaximized();
    document.documentElement.classList.toggle("window-maximized", maximized.value);
  });
  systemDark.addEventListener("change", onSystemThemeChange);
  unlisten = await Promise.all([
    listen<EnvironmentStatus>("proxy-state-changed", ({ payload }) => { environment.value = payload; }),
    listen<string>("operation-error", ({ payload }) => { error.value = payload; })
  ]);
  await refresh();
  refreshTimer = window.setInterval(() => void refresh(true), 5000);
});

onBeforeUnmount(() => {
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
    <AppHeader
      :copy="copy"
      :maximized="maximized"
      :view="view"
      @close-settings="closeSettings"
      @open-settings="openSettings"
      @minimize="minimizeWindow"
      @toggle-maximize="toggleMaximizeWindow"
      @close="closeWindow"
    />

    <Transition name="view-fade" mode="out-in">
    <ProxyPage
      v-if="view === 'home'"
      key="home"
      :copy="copy"
      :environment="environment"
      :detected="detected"
      :endpoint="endpoint"
      :error="error"
      :loading="loading"
      :toggling="toggling"
      :copied-endpoint="copiedEndpoint"
      :selected-variables="draftSettings.proxyVariables"
      @refresh="refresh(false)"
      @toggle="toggle"
      @copy-endpoint="copyEndpoint"
      @toggle-variable="toggleManagedVariable"
    />

    <SettingsPage
      v-else
      key="settings"
      v-model:settings="draftSettings"
      v-model:tab="settingsTab"
      :copy="copy"
      :settings-error="settingsError"
      :settings-load-error="settingsLoadError"
      :app-version="appVersion"
      :update-state="updateState"
      :update-message="updateMessage"
      @check-for-updates="checkForUpdates"
    />
    </Transition>
  </div>
</template>
