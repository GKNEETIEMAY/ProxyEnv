<script setup lang="ts">
import { getVersion } from "@tauri-apps/api/app";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { messages, resolveLocale } from "./i18n";
import { backend } from "./services/backend";
import type { AppSettings, EnvironmentStatus, ManagedProxyVariable, ProxyCandidate } from "./types";

const defaultSettings: AppSettings = {
  language: "system",
  theme: "system",
  launchAtStartup: false,
  silentStart: false,
  closeToTray: true,
  proxyVariables: ["http", "https"]
};

const view = ref<"home" | "settings">("home");
const settingsTab = ref<"general" | "about">("general");
const loading = ref(true);
const toggling = ref(false);
const error = ref("");
const settingsError = ref("");
const settingsLoadError = ref("");
const copiedEndpoint = ref(false);
const appVersion = ref("0.1.0");
const latestVersion = ref("");
const updateState = ref<"idle" | "checking" | "latest" | "available" | "unpublished" | "error">("idle");
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
const activeCount = computed(() => environment.value.entries.filter((entry) => entry.exists).length);
const clientIcons: Record<string, string> = {
  "clash-verge-rev": "/proxy-clients/clash-verge-rev.png",
  v2rayn: "/proxy-clients/v2rayn.png",
  flclash: "/proxy-clients/flclash.ico",
  hiddify: "/proxy-clients/hiddify.ico",
  "clash-nyanpasu": "/proxy-clients/clash-nyanpasu.png",
  "generic-proxy": "/proxy-clients/generic-proxy.svg"
};
const detectedIcon = computed(() => detected.value
  ? clientIcons[detected.value.iconKey ?? ""] ?? clientIcons["generic-proxy"]
  : clientIcons["generic-proxy"]);
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

function variableDescription(name: string): string {
  const normalized = name.toUpperCase();
  if (normalized === "HTTP_PROXY") return copy.value.httpProxyDescription;
  if (normalized === "HTTPS_PROXY") return copy.value.httpsProxyDescription;
  if (normalized === "ALL_PROXY") return copy.value.allProxyDescription;
  return copy.value.noProxyDescription;
}

function variableActionLabel(name: string, action: "write" | "about"): string {
  const template = action === "write" ? copy.value.writeVariable : copy.value.aboutVariable;
  return template.replace("{name}", name);
}

function isManagedVariableSelected(name: string): boolean {
  const key = managedVariableKey(name);
  return key !== undefined && draftSettings.value.proxyVariables.includes(key);
}

function isLastManagedVariable(name: string): boolean {
  return isManagedVariableSelected(name) && draftSettings.value.proxyVariables.length === 1;
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
    <header class="app-header">
      <div v-if="view === 'settings'" class="settings-header-context">
        <button class="header-back-button" type="button" :aria-label="copy.back" @click="closeSettings">
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m12.5 5-5 5 5 5" /></svg>
        </button>
        <strong>{{ copy.settingsTitle }}</strong>
      </div>
      <button v-else class="wordmark" type="button" @click="closeSettings">
        <span class="brand-symbol" aria-hidden="true">
          <svg viewBox="0 0 28 28">
            <path class="brand-arrow-blue" d="M4.5 9.4h13.2M14.4 5.2l4.2 4.2-4.2 4.2" />
            <path class="brand-arrow-white" d="M23.5 18.6H10.3M13.6 22.8l-4.2-4.2 4.2-4.2" />
          </svg>
        </span>
        <span><strong>{{ copy.appName }}</strong><small>{{ copy.appTagline }}</small></span>
      </button>
      <div class="titlebar-drag-zone" data-tauri-drag-region @dblclick="toggleMaximizeWindow"></div>
      <div class="header-actions">
        <div v-if="view === 'home'" class="primary-nav">
          <button class="settings-nav-button" type="button" :aria-label="copy.settings" :title="copy.settings" @click="openSettings">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M12.2 3h-.4a1.8 1.8 0 0 0-1.8 1.8v.3a1.8 1.8 0 0 1-.9 1.56l-.6.34a1.8 1.8 0 0 1-1.8 0l-.25-.14a1.8 1.8 0 0 0-2.46.66l-.2.35a1.8 1.8 0 0 0 .66 2.46l.25.15a1.8 1.8 0 0 1 .9 1.56v.68a1.8 1.8 0 0 1-.9 1.56l-.25.15a1.8 1.8 0 0 0-.66 2.46l.2.35a1.8 1.8 0 0 0 2.46.66l.25-.14a1.8 1.8 0 0 1 1.8 0l.6.34a1.8 1.8 0 0 1 .9 1.56v.3a1.8 1.8 0 0 0 1.8 1.8h.4a1.8 1.8 0 0 0 1.8-1.8v-.3a1.8 1.8 0 0 1 .9-1.56l.6-.34a1.8 1.8 0 0 1 1.8 0l.25.14a1.8 1.8 0 0 0 2.46-.66l.2-.35a1.8 1.8 0 0 0-.66-2.46l-.25-.15a1.8 1.8 0 0 1-.9-1.56v-.68a1.8 1.8 0 0 1 .9-1.56l.25-.15a1.8 1.8 0 0 0 .66-2.46l-.2-.35a1.8 1.8 0 0 0-2.46-.66l-.25.14a1.8 1.8 0 0 1-1.8 0l-.6-.34a1.8 1.8 0 0 1-.9-1.56v-.3A1.8 1.8 0 0 0 12.2 3Z" />
              <circle cx="12" cy="12.4" r="2.7" />
            </svg>
          </button>
        </div>
        <div class="window-controls">
          <button type="button" :aria-label="copy.minimizeWindow" @click="minimizeWindow">
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 11.5h10" /></svg>
          </button>
          <button type="button" :aria-label="maximized ? copy.restoreWindow : copy.maximizeWindow" @click="toggleMaximizeWindow">
            <svg v-if="!maximized" viewBox="0 0 16 16" aria-hidden="true"><rect x="3.5" y="3.5" width="9" height="9" rx=".6" /></svg>
            <svg v-else viewBox="0 0 16 16" aria-hidden="true"><path d="M5.5 5.5V3.8h6.7v6.7h-1.7M3.8 5.5h6.7v6.7H3.8z" /></svg>
          </button>
          <button class="window-close" type="button" :aria-label="copy.closeWindow" @click="closeWindow">
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m4 4 8 8m0-8-8 8" /></svg>
          </button>
        </div>
      </div>
    </header>

    <Transition name="view-fade" mode="out-in">
    <main v-if="view === 'home'" key="home" class="page home-page">
      <div v-if="error" class="notice notice-error" role="alert">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 8v5m0 3.5v.01M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" /></svg>
        <p><strong>{{ copy.operationFailed }}</strong><span>{{ error }} · {{ copy.retryHint }}</span></p>
      </div>
      <div v-if="environment.warning" class="notice notice-warning">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 8v5m0 3.5v.01M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" /></svg>
        <p><strong>{{ copy.attention }}</strong><span>{{ environment.warning }}</span></p>
      </div>

      <section class="proxy-stage" :class="{ enabled: environment.enabled }">
        <div class="proxy-identity">
          <span class="section-title">{{ copy.currentProxy }}</span>
          <div v-if="detected" class="client-heading">
            <span class="client-art"><img :src="detectedIcon" alt="" /></span>
            <div>
              <h1>{{ detected.clientName || copy.localProxy }}</h1>
              <p><span class="status-dot" :class="{ quiet: !detected.listening }"></span>{{ detected.listening ? copy.listening : copy.notListening }}</p>
            </div>
          </div>
          <div v-else class="empty-state">
            <span class="client-art generic"><img :src="detectedIcon" alt="" /></span>
            <div><h1>{{ loading ? copy.detecting : copy.noProxy }}</h1><p>{{ copy.noProxyHint }}</p></div>
          </div>
          <div v-if="detected" class="endpoint-line">
            <div class="endpoint-address">
              <code>{{ endpoint }}</code>
              <button type="button" :aria-label="copiedEndpoint ? copy.endpointCopied : copy.copyEndpoint" :title="copiedEndpoint ? copy.endpointCopied : copy.copyEndpoint" @click="copyEndpoint">
                <svg v-if="!copiedEndpoint" viewBox="0 0 20 20" aria-hidden="true"><rect x="6.5" y="6.5" width="9" height="9" rx="1.6"/><path d="M13.5 6.5V5A1.5 1.5 0 0 0 12 3.5H5A1.5 1.5 0 0 0 3.5 5v7A1.5 1.5 0 0 0 5 13.5h1.5"/></svg>
                <svg v-else viewBox="0 0 20 20" aria-hidden="true"><path d="m4.5 10.2 3.2 3.2 7.8-7.8"/></svg>
              </button>
            </div>
            <span>{{ detected.protocol }} · {{ detected.confidence }} {{ copy.autoConfidence }}</span>
          </div>
        </div>

        <div class="proxy-action">
          <div>
            <span class="section-title">{{ copy.proxyEnvironment }}</span>
            <strong>{{ environment.enabled ? copy.enabled : copy.disabled }}</strong>
            <p>{{ environment.enabled ? copy.environmentOnHint : copy.environmentOffHint }}</p>
          </div>
          <button class="toggle-control" :class="{ active: environment.enabled }" type="button" :disabled="loading || toggling" :aria-pressed="environment.enabled" @click="toggle">
            <span class="toggle-track"><span></span></span>
            <b>{{ toggling ? copy.enabling : environment.enabled ? copy.enabled : copy.disabled }}</b>
          </button>
        </div>
      </section>

      <section class="variables-section">
        <div class="section-header">
          <div><h2>{{ copy.variables }}</h2><p>{{ copy.variablesHint }}</p></div>
          <button class="refresh-button" type="button" :disabled="loading" @click="refresh(false)">
            <svg :class="{ spinning: loading }" viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11a8 8 0 1 0-2.34 5.66M20 5v6h-6" /></svg>{{ copy.refresh }}
          </button>
        </div>
        <div v-if="environment.entries.length" class="variable-table">
          <div v-for="entry in environment.entries" :key="entry.name" class="variable-row">
            <span class="variable-indicator" :class="{ active: entry.exists }"></span>
            <div class="variable-label">
              <code class="variable-name">{{ entry.name }}</code>
              <span
                class="variable-help"
                tabindex="0"
                :aria-label="variableActionLabel(entry.name, 'about')"
                :aria-describedby="`variable-help-${entry.name}`"
                @mousedown.prevent
              >
                <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="10" cy="10" r="7.5"/><path d="M7.9 7.8a2.25 2.25 0 0 1 4.3.9c0 1.55-2.2 1.75-2.2 3.2M10 14.6v.01"/></svg>
                <p :id="`variable-help-${entry.name}`" role="tooltip">{{ variableDescription(entry.name) }}</p>
              </span>
            </div>
            <code class="variable-value" :class="{ unset: !entry.exists }">{{ entry.value ?? copy.unset }}</code>
            <label v-if="managedVariableKey(entry.name)" class="variable-choice" :class="{ locked: isLastManagedVariable(entry.name) }">
              <input
                type="checkbox"
                :checked="isManagedVariableSelected(entry.name)"
                :disabled="isLastManagedVariable(entry.name)"
                :aria-label="variableActionLabel(entry.name, 'write')"
                @change="toggleManagedVariable(entry.name)"
              />
              <span aria-hidden="true"><svg viewBox="0 0 16 16"><path d="m4 8.2 2.5 2.5L12 5.5"/></svg></span>
            </label>
            <span v-else class="variable-choice-spacer"></span>
          </div>
        </div>
        <div v-else class="skeleton-list" :aria-label="copy.detecting"><span v-for="item in 4" :key="item"></span></div>
        <p class="table-caption">{{ activeCount }} / {{ environment.entries.length }} · {{ copy.localOnly }}</p>
      </section>
    </main>

    <main v-else key="settings" class="page settings-page">
      <nav class="settings-tabs" :aria-label="copy.settingsTitle">
        <button type="button" :class="{ active: settingsTab === 'general' }" :aria-current="settingsTab === 'general' ? 'page' : undefined" @click="settingsTab = 'general'">{{ copy.general }}</button>
        <button type="button" :class="{ active: settingsTab === 'about' }" :aria-current="settingsTab === 'about' ? 'page' : undefined" @click="settingsTab = 'about'">{{ copy.about }}</button>
      </nav>

      <p class="settings-intro">{{ settingsTab === 'general' ? copy.settingsIntro : copy.aboutIntro }}</p>

      <div v-if="settingsLoadError || settingsError" class="notice notice-error" role="alert">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 8v5m0 3.5v.01M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" /></svg>
        <p><strong>{{ settingsLoadError ? copy.settingsLoadFailed : copy.saveFailed }}</strong><span>{{ settingsLoadError || settingsError }}</span></p>
      </div>

      <template v-if="settingsTab === 'general'">
      <section class="settings-group">
        <div class="group-heading"><h2>{{ copy.appearance }}</h2><p>{{ copy.appearanceHint }}</p></div>
        <div class="setting-row setting-row-select">
          <div><label for="language">{{ copy.interfaceLanguage }}</label><p>{{ copy.languageHint }}</p></div>
          <div class="select-wrap">
            <select id="language" v-model="draftSettings.language">
              <option value="system">{{ copy.system }}</option><option value="zh-CN">{{ copy.chinese }}</option><option value="en">{{ copy.english }}</option><option value="ja">{{ copy.japanese }}</option><option value="ko">{{ copy.korean }}</option>
            </select>
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m8 10 4 4 4-4" /></svg>
          </div>
        </div>
        <div class="setting-row setting-row-stack">
          <div><span class="setting-label">{{ copy.theme }}</span></div>
          <div class="theme-options" role="radiogroup" :aria-label="copy.theme">
            <label><input v-model="draftSettings.theme" type="radio" value="system" /><span><svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="13" rx="2"/><path d="M8 21h8m-4-4v4"/></svg>{{ copy.themeSystem }}</span></label>
            <label><input v-model="draftSettings.theme" type="radio" value="light" /><span><svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M4.93 4.93l1.42 1.42m11.3 11.3 1.42 1.42M2 12h2m16 0h2M4.93 19.07l1.42-1.42m11.3-11.3 1.42-1.42"/></svg>{{ copy.themeLight }}</span></label>
            <label><input v-model="draftSettings.theme" type="radio" value="dark" /><span><svg viewBox="0 0 24 24"><path d="M20.5 14.2A8.5 8.5 0 0 1 9.8 3.5a8.5 8.5 0 1 0 10.7 10.7Z"/></svg>{{ copy.themeDark }}</span></label>
          </div>
        </div>
      </section>

      <section class="settings-group">
        <div class="group-heading"><h2>{{ copy.windowBehavior }}</h2><p>{{ copy.windowBehaviorHint }}</p></div>
        <label class="setting-row boolean-row">
          <span><strong>{{ copy.launchAtStartup }}</strong><small>{{ copy.launchAtStartupHint }}</small></span>
          <input v-model="draftSettings.launchAtStartup" class="switch-input" type="checkbox" />
        </label>
        <label class="setting-row boolean-row" :class="{ muted: !draftSettings.launchAtStartup }">
          <span><strong>{{ copy.silentStart }}</strong><small>{{ copy.silentStartHint }}</small></span>
          <input v-model="draftSettings.silentStart" class="switch-input" type="checkbox" :disabled="!draftSettings.launchAtStartup" />
        </label>
        <label class="setting-row boolean-row">
          <span><strong>{{ copy.closeToTray }}</strong><small>{{ copy.closeToTrayHint }}</small></span>
          <input v-model="draftSettings.closeToTray" class="switch-input" type="checkbox" />
        </label>
      </section>

      </template>

      <section v-else class="about-panel">
        <div class="about-identity">
          <span class="brand-symbol about-symbol" aria-hidden="true">
            <svg viewBox="0 0 28 28">
              <path class="brand-arrow-blue" d="M4.5 9.4h13.2M14.4 5.2l4.2 4.2-4.2 4.2" />
              <path class="brand-arrow-white" d="M23.5 18.6H10.3M13.6 22.8l-4.2-4.2 4.2-4.2" />
            </svg>
          </span>
          <div><h2>{{ copy.appName }}</h2><p>{{ copy.appTagline }}</p></div>
        </div>
        <dl class="about-details">
          <div><dt>{{ copy.version }}</dt><dd>v{{ appVersion }}</dd></div>
          <div><dt>{{ copy.updateStatus }}</dt><dd :class="`update-${updateState}`"><span class="update-dot"></span>{{ updateMessage }}</dd></div>
          <div><dt>{{ copy.updateSource }}</dt><dd>GitHub Releases</dd></div>
        </dl>
        <button class="check-update-button" type="button" :disabled="updateState === 'checking'" @click="checkForUpdates">
          <svg :class="{ spinning: updateState === 'checking' }" viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11a8 8 0 1 0-2.34 5.66M20 5v6h-6" /></svg>
          {{ updateState === 'checking' ? copy.checkingUpdates : copy.checkForUpdates }}
        </button>
        <section class="changelog-section">
          <div class="changelog-heading"><h3>{{ copy.changelog }}</h3><span>v{{ appVersion }} · {{ copy.developmentPreview }}</span></div>
          <ul>
            <li>{{ copy.changelogDiscovery }}</li>
            <li>{{ copy.changelogVariables }}</li>
            <li>{{ copy.changelogDesktop }}</li>
          </ul>
        </section>
      </section>

    </main>
    </Transition>
  </div>
</template>
