<script setup lang="ts">
import type { Copy } from "../../shared/i18n";

defineProps<{
  copy: Copy;
  maximized: boolean;
  view: "home" | "settings";
}>();

defineEmits<{
  closeSettings: [];
  openSettings: [];
  minimize: [];
  toggleMaximize: [];
  close: [];
}>();
</script>

<template>
  <header class="app-header">
    <div v-if="view === 'settings'" class="settings-header-context">
      <button class="header-back-button" type="button" :aria-label="copy.back" @click="$emit('closeSettings')">
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m12.5 5-5 5 5 5" /></svg>
      </button>
      <strong>{{ copy.settingsTitle }}</strong>
    </div>
    <button v-else class="wordmark" type="button" @click="$emit('closeSettings')">
      <span class="brand-symbol" aria-hidden="true">
        <svg viewBox="0 0 28 28">
          <path class="brand-arrow-blue" d="M4.5 9.4h13.2M14.4 5.2l4.2 4.2-4.2 4.2" />
          <path class="brand-arrow-white" d="M23.5 18.6H10.3M13.6 22.8l-4.2-4.2 4.2-4.2" />
        </svg>
      </span>
      <span><strong>{{ copy.appName }}</strong><small>{{ copy.appTagline }}</small></span>
    </button>
    <div class="titlebar-drag-zone" data-tauri-drag-region @dblclick="$emit('toggleMaximize')"></div>
    <div class="header-actions">
      <div v-if="view === 'home'" class="primary-nav">
        <button class="settings-nav-button" type="button" :aria-label="copy.settings" :title="copy.settings" @click="$emit('openSettings')">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M12.2 3h-.4a1.8 1.8 0 0 0-1.8 1.8v.3a1.8 1.8 0 0 1-.9 1.56l-.6.34a1.8 1.8 0 0 1-1.8 0l-.25-.14a1.8 1.8 0 0 0-2.46.66l-.2.35a1.8 1.8 0 0 0 .66 2.46l.25.15a1.8 1.8 0 0 1 .9 1.56v.68a1.8 1.8 0 0 1-.9 1.56l-.25.15a1.8 1.8 0 0 0-.66 2.46l.2.35a1.8 1.8 0 0 0 2.46.66l.25-.14a1.8 1.8 0 0 1 1.8 0l.6.34a1.8 1.8 0 0 1 .9 1.56v.3a1.8 1.8 0 0 0 1.8 1.8h.4a1.8 1.8 0 0 0 1.8-1.8v-.3a1.8 1.8 0 0 1 .9-1.56l.6-.34a1.8 1.8 0 0 1 1.8 0l.25.14a1.8 1.8 0 0 0 2.46-.66l.2-.35a1.8 1.8 0 0 0-.66-2.46l-.25-.15a1.8 1.8 0 0 1-.9-1.56v-.68a1.8 1.8 0 0 1 .9-1.56l.25-.15a1.8 1.8 0 0 0 .66-2.46l-.2-.35a1.8 1.8 0 0 0-2.46-.66l-.25.14a1.8 1.8 0 0 1-1.8 0l-.6-.34a1.8 1.8 0 0 1-.9-1.56v-.3A1.8 1.8 0 0 0 12.2 3Z" />
            <circle cx="12" cy="12.4" r="2.7" />
          </svg>
        </button>
      </div>
      <div class="window-controls">
        <button type="button" :aria-label="copy.minimizeWindow" @click="$emit('minimize')">
          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 11.5h10" /></svg>
        </button>
        <button type="button" :aria-label="maximized ? copy.restoreWindow : copy.maximizeWindow" @click="$emit('toggleMaximize')">
          <svg v-if="!maximized" viewBox="0 0 16 16" aria-hidden="true"><rect x="3.5" y="3.5" width="9" height="9" rx=".6" /></svg>
          <svg v-else viewBox="0 0 16 16" aria-hidden="true"><path d="M5.5 5.5V3.8h6.7v6.7h-1.7M3.8 5.5h6.7v6.7H3.8z" /></svg>
        </button>
        <button class="window-close" type="button" :aria-label="copy.closeWindow" @click="$emit('close')">
          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m4 4 8 8m0-8-8 8" /></svg>
        </button>
      </div>
    </div>
  </header>
</template>
