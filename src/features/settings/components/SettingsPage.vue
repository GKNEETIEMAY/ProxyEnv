<script setup lang="ts">
import type { Copy } from "../../../shared/i18n";
import type { AppSettings } from "../../../shared/types";
import type { ReleaseNoteLine, UpdateState } from "../update";

export type SettingsTab = "general" | "about";

defineProps<{
  copy: Copy;
  settingsError: string;
  settingsLoadError: string;
  appVersion: string;
  updateState: UpdateState;
  updateMessage: string;
  releaseVersion: string;
  releasePublishedLabel: string;
  releaseNotes: ReleaseNoteLine[];
  releaseUrl: string;
  releaseActionError: string;
}>();

const settings = defineModel<AppSettings>("settings", { required: true });
const tab = defineModel<SettingsTab>("tab", { required: true });

defineEmits<{ checkForUpdates: []; openRelease: [] }>();
</script>

<template>
  <main class="page settings-page">
    <nav class="settings-tabs" :aria-label="copy.settingsTitle">
      <button type="button" :class="{ active: tab === 'general' }" :aria-current="tab === 'general' ? 'page' : undefined" @click="tab = 'general'">{{ copy.general }}</button>
      <button type="button" :class="{ active: tab === 'about' }" :aria-current="tab === 'about' ? 'page' : undefined" @click="tab = 'about'">{{ copy.about }}</button>
    </nav>

    <p class="settings-intro">{{ tab === 'general' ? copy.settingsIntro : copy.aboutIntro }}</p>

    <div v-if="settingsLoadError || settingsError" class="notice notice-error" role="alert">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 8v5m0 3.5v.01M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" /></svg>
      <p><strong>{{ settingsLoadError ? copy.settingsLoadFailed : copy.saveFailed }}</strong><span>{{ settingsLoadError || settingsError }}</span></p>
    </div>

    <template v-if="tab === 'general'">
      <section class="settings-group">
        <div class="group-heading"><h2>{{ copy.appearance }}</h2><p>{{ copy.appearanceHint }}</p></div>
        <div class="setting-row setting-row-select">
          <div><label for="language">{{ copy.interfaceLanguage }}</label><p>{{ copy.languageHint }}</p></div>
          <div class="select-wrap">
            <select id="language" v-model="settings.language">
              <option value="system">{{ copy.system }}</option><option value="zh-CN">{{ copy.chinese }}</option><option value="en">{{ copy.english }}</option><option value="ja">{{ copy.japanese }}</option><option value="ko">{{ copy.korean }}</option>
            </select>
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m8 10 4 4 4-4" /></svg>
          </div>
        </div>
        <div class="setting-row setting-row-stack">
          <div><span class="setting-label">{{ copy.theme }}</span></div>
          <div class="theme-options" role="radiogroup" :aria-label="copy.theme">
            <label><input v-model="settings.theme" type="radio" value="system" /><span><svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="13" rx="2"/><path d="M8 21h8m-4-4v4"/></svg>{{ copy.themeSystem }}</span></label>
            <label><input v-model="settings.theme" type="radio" value="light" /><span><svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M4.93 4.93l1.42 1.42m11.3 11.3 1.42 1.42M2 12h2m16 0h2M4.93 19.07l1.42-1.42m11.3-11.3 1.42-1.42"/></svg>{{ copy.themeLight }}</span></label>
            <label><input v-model="settings.theme" type="radio" value="dark" /><span><svg viewBox="0 0 24 24"><path d="M20.5 14.2A8.5 8.5 0 0 1 9.8 3.5a8.5 8.5 0 1 0 10.7 10.7Z"/></svg>{{ copy.themeDark }}</span></label>
          </div>
        </div>
      </section>

      <section class="settings-group">
        <div class="group-heading"><h2>{{ copy.windowBehavior }}</h2><p>{{ copy.windowBehaviorHint }}</p></div>
        <label class="setting-row boolean-row">
          <span><strong>{{ copy.launchAtStartup }}</strong><small>{{ copy.launchAtStartupHint }}</small></span>
          <input v-model="settings.launchAtStartup" class="switch-input" type="checkbox" />
        </label>
        <label class="setting-row boolean-row" :class="{ muted: !settings.launchAtStartup }">
          <span><strong>{{ copy.silentStart }}</strong><small>{{ copy.silentStartHint }}</small></span>
          <input v-model="settings.silentStart" class="switch-input" type="checkbox" :disabled="!settings.launchAtStartup" />
        </label>
        <label class="setting-row boolean-row">
          <span><strong>{{ copy.closeToTray }}</strong><small>{{ copy.closeToTrayHint }}</small></span>
          <input v-model="settings.closeToTray" class="switch-input" type="checkbox" />
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
      <div class="update-actions">
        <button class="check-update-button" type="button" :disabled="updateState === 'checking'" @click="$emit('checkForUpdates')">
          <svg :class="{ spinning: updateState === 'checking' }" viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11a8 8 0 1 0-2.34 5.66M20 5v6h-6" /></svg>
          {{ updateState === 'checking' ? copy.checkingUpdates : copy.checkForUpdates }}
        </button>
        <button v-if="releaseUrl" class="release-link-button" type="button" @click="$emit('openRelease')">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14 5h5v5m0-5-8 8M19 13v5a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h5" /></svg>
          {{ updateState === 'available' ? copy.downloadUpdate : copy.viewRelease }}
        </button>
      </div>
      <p v-if="releaseActionError" class="release-action-error" role="alert">{{ releaseActionError }}</p>
      <section class="changelog-section">
        <div class="changelog-heading">
          <h3>{{ copy.changelog }}</h3>
          <span>v{{ releaseVersion }} · {{ releasePublishedLabel || copy.stableRelease }}</span>
        </div>
        <div class="release-notes" aria-live="polite">
          <template v-for="(line, index) in releaseNotes" :key="`${line.kind}-${index}-${line.text}`">
            <h4 v-if="line.kind === 'heading'">{{ line.text }}</h4>
            <p v-else :class="`release-note-${line.kind}`">{{ line.text }}</p>
          </template>
        </div>
      </section>
    </section>
  </main>
</template>
