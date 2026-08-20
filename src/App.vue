<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { backend } from "./services/backend";
import type { EnvironmentStatus, ProxyCandidate } from "./types";

const loading = ref(true);
const toggling = ref(false);
const error = ref("");
const environment = ref<EnvironmentStatus>({ enabled: false, entries: [] });
const candidates = ref<ProxyCandidate[]>([]);
let refreshTimer: number | undefined;
let refreshPending = false;

const detected = computed(() => candidates.value[0]);
const clientIcons: Record<string, string> = {
  "clash-verge-rev": "/proxy-clients/clash-verge-rev.png",
  "v2rayn": "/proxy-clients/v2rayn.png",
  "flclash": "/proxy-clients/flclash.ico",
  "hiddify": "/proxy-clients/hiddify.ico",
  "clash-nyanpasu": "/proxy-clients/clash-nyanpasu.png",
  "generic-proxy": "/proxy-clients/generic-proxy.svg"
};
const detectedIcon = computed(() => detected.value
  ? clientIcons[detected.value.iconKey ?? ""] ?? clientIcons["generic-proxy"]
  : undefined);

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

onMounted(() => {
  void refresh();
  refreshTimer = window.setInterval(() => void refresh(true), 5000);
});

onBeforeUnmount(() => {
  if (refreshTimer !== undefined) window.clearInterval(refreshTimer);
});
</script>

<template>
  <main class="app-shell">
    <header class="topbar">
      <div class="brand">
        <span class="brand-mark" aria-hidden="true">
          <svg viewBox="0 0 24 24"><path d="M4 8h11l-2.5-2.5L14 4l5 5-5 5-1.5-1.5L15 10H4V8Zm16 8H9l2.5 2.5L10 20l-5-5 5-5 1.5 1.5L9 14h11v2Z" /></svg>
        </span>
        <div>
          <h1>境启 <span>ProxyEnv</span></h1>
          <p>本机代理环境控制</p>
        </div>
      </div>
      <button class="icon-button" :class="{ spinning: loading }" :disabled="loading" title="刷新状态" @click="refresh(false)">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M17.65 6.35A7.95 7.95 0 0 0 12 4a8 8 0 1 0 7.75 10h-2.1A6 6 0 1 1 12 6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35Z" /></svg>
      </button>
    </header>

    <div v-if="error" class="notice error-notice">
      <span>!</span><p><strong>操作失败</strong>{{ error }}</p>
    </div>
    <div v-if="environment.warning" class="notice warning-notice">
      <span>!</span><p><strong>需要注意</strong>{{ environment.warning }}</p>
    </div>

    <section class="overview-card">
      <div class="proxy-overview">
        <p class="section-label">当前代理</p>
        <template v-if="detected">
          <div class="proxy-name">
            <img v-if="detectedIcon" class="client-icon" :src="detectedIcon" :alt="`${detected.clientName} 图标`" />
            <span class="live-dot" :class="{ offline: !detected.listening }"></span>
            {{ detected.clientName || "本机代理" }}
          </div>
          <p class="endpoint">{{ detected.host }}:{{ detected.port }}</p>
          <p class="metadata">{{ detected.protocol }} · {{ detected.confidence }} confidence</p>
        </template>
        <div v-else class="empty-proxy">
          <span class="empty-icon">—</span>
          <div>
            <strong>{{ loading ? "正在检测" : "未检测到代理" }}</strong>
            <p>{{ loading ? "读取本机代理状态…" : "启动代理客户端后点击右上角刷新" }}</p>
          </div>
        </div>
      </div>

      <div class="environment-control">
        <div class="control-copy">
          <p class="section-label">代理环境变量</p>
          <strong>{{ environment.enabled ? "已开启" : "已关闭" }}</strong>
          <span>{{ environment.enabled ? "新启动的程序将继承代理" : "新启动的程序将直连网络" }}</span>
        </div>
        <button class="switch" :class="{ enabled: environment.enabled }" :disabled="loading || toggling" :aria-pressed="environment.enabled" @click="toggle">
          <span class="switch-track"><span class="switch-thumb"></span></span>
          <span class="switch-label">{{ toggling ? "处理中" : environment.enabled ? "ON" : "OFF" }}</span>
        </button>
      </div>
    </section>

    <section class="variables-card">
      <div class="section-heading">
        <div><h2>环境变量</h2><p>仅修改当前 Windows 用户</p></div>
        <span class="variable-count">{{ environment.entries.filter((entry) => entry.exists).length }} / {{ environment.entries.length }}</span>
      </div>
      <div v-if="environment.entries.length" class="variable-list">
        <div v-for="entry in environment.entries" :key="entry.name" class="variable-row">
          <span class="variable-status" :class="{ active: entry.exists }"></span>
          <strong>{{ entry.name }}</strong>
          <code :class="{ unset: !entry.exists }">{{ entry.value ?? "未设置" }}</code>
        </div>
      </div>
      <div v-else class="skeleton-list" aria-label="正在读取环境变量"><span v-for="item in 4" :key="item"></span></div>
    </section>

    <footer>数据仅在本机处理 · 不修改 Windows 系统代理</footer>
  </main>
</template>
