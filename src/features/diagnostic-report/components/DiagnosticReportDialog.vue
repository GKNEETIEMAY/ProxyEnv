<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { backend } from "../../../shared/api/backend";
import { messages, type Copy, type Locale } from "../../../shared/i18n";
import type { DiagnosticReportData } from "../../../shared/types";
import { copyText } from "../../../shared/utils/clipboard";
import { formatDiagnosticReport } from "../format";

// Operate: extend the warm native-dialog vocabulary. Language, readable snapshot, then Copy.
// Keep a stable preview height; read-only collection never writes files or starts network tests.
const props = defineProps<{ copy: Copy; locale: Locale; applicationId?: string; reviewPreview?: boolean }>();
const dialog = ref<HTMLDialogElement>();
const language = ref<Locale | "interface">("interface");
const data = ref<DiagnosticReportData>();
const loading = ref(false);
const failed = ref(false);
const copyFailed = ref(false);
const copied = ref(false);
let requestId = 0;
const reportLocale = computed(() => language.value === "interface" ? props.locale : language.value);
const report = computed(() => data.value ? formatDiagnosticReport(data.value, messages[reportLocale.value]) : "");

async function refresh() {
  const request = ++requestId;
  data.value = undefined;
  failed.value = false;
  loading.value = true;
  try {
    const snapshot: DiagnosticReportData = import.meta.env.DEV && props.reviewPreview ? {
      appVersion: "0.1.3", os: "windows", osVersion: "11 (26100)", detectedCount: 2, listeningCount: 1,
      selectedClient: "Clash Verge Rev", hasSelection: true, available: true, protocol: "mixed", confidence: "high",
      systemProxyEnabled: false, environment: "enabled", managedVariables: ["http", "https"], tun: "possible",
      connectivity: "partial", successfulTargets: 2, totalTargets: 3, errorCategories: ["connectTimeout"],
      assistant: { category: "unrecognized", state: "environmentConfigured", action: "none" }
    } : await backend.diagnosticReport(props.applicationId);
    if (request === requestId) data.value = snapshot;
  } catch {
    if (request === requestId) failed.value = true;
  } finally {
    if (request === requestId) loading.value = false;
  }
}

function open() {
  if (dialog.value?.open) return;
  language.value = "interface";
  copied.value = false;
  copyFailed.value = false;
  dialog.value?.showModal();
  void refresh();
}

function close() {
  ++requestId;
  loading.value = false;
  data.value = undefined;
  dialog.value?.close();
}

async function copyReport() {
  if (!report.value || loading.value) return;
  const text = report.value;
  copyFailed.value = false;
  try {
    await copyText(text);
    if (report.value === text) copied.value = true;
  } catch {
    copyFailed.value = true;
  }
}

watch(report, () => { copied.value = false; copyFailed.value = false; });
// A different selected app must never leave the previous app's report on screen.
watch(() => props.applicationId, () => { if (dialog.value?.open) void refresh(); });
onBeforeUnmount(() => { ++requestId; });
defineExpose({ open });
</script>

<template>
  <dialog ref="dialog" class="confirmation-dialog diagnostic-report-dialog" aria-labelledby="diagnostic-report-title" aria-describedby="diagnostic-report-privacy" @cancel.prevent="close">
    <form @submit.prevent="copyReport">
      <h2 id="diagnostic-report-title">{{ copy.reportOpen }}</h2>
      <p id="diagnostic-report-privacy">{{ copy.reportPrivacy }}</p>
      <div class="report-language-row">
        <label for="report-language">{{ copy.reportLanguage }}</label>
        <select id="report-language" v-model="language" autofocus>
          <option value="interface">{{ copy.reportFollowInterface }}</option>
          <option value="zh-CN">简体中文</option>
          <option value="en">English</option>
          <option value="ja">日本語</option>
          <option value="ko">한국어</option>
        </select>
      </div>
      <textarea class="report-preview" :value="report" :lang="reportLocale" :aria-label="copy.reportPreview" :aria-busy="loading" readonly spellcheck="false"></textarea>
      <div class="report-feedback" aria-live="polite">
        <span v-if="loading">{{ copy.reportLoading }}</span>
        <span v-else-if="failed" class="report-error" role="alert">{{ copy.reportLoadFailed }}</span>
        <span v-else-if="copyFailed" class="report-error" role="alert">{{ copy.reportCopyFailed }}</span>
        <span v-else-if="copied">{{ copy.reportCopied }}</span>
        <span v-else>{{ copy.reportSnapshot }}</span>
      </div>
      <div class="confirmation-actions">
        <button class="secondary-action" type="button" @click="close">{{ copy.assistantClose }}</button>
        <button class="secondary-action" type="button" :disabled="loading" @click="refresh">{{ copy.reportRefresh }}</button>
        <button class="primary-action" type="submit" :disabled="loading || !data">{{ copy.reportCopy }}</button>
      </div>
    </form>
  </dialog>
</template>
