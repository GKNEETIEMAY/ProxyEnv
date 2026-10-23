<script setup lang="ts">
import { computed } from "vue";
import type { Copy } from "../../../shared/i18n";
import type { EnvironmentStatus, ManagedProxyVariable } from "../../../shared/types";
import HelpTooltip from "../../../shared/components/HelpTooltip.vue";
const props = defineProps<{ copy: Copy; environment: EnvironmentStatus; selectedVariables: ManagedProxyVariable[]; toggling: boolean; editable: boolean }>();
const emit = defineEmits<{ toggleVariable: [name: string] }>();
const visibleEntries = computed(() => props.environment.entries.filter(entry => props.editable || managedVariableKey(entry.name) !== undefined));
function managedVariableKey(name: string): ManagedProxyVariable | undefined { const normalized = name.toUpperCase(); if (normalized === "HTTP_PROXY") return "http"; if (normalized === "HTTPS_PROXY") return "https"; if (normalized === "ALL_PROXY") return "all"; return undefined; }
function variableDescription(name: string): string { const normalized = name.toUpperCase(); if (normalized === "HTTP_PROXY") return props.copy.httpProxyDescription; if (normalized === "HTTPS_PROXY") return props.copy.httpsProxyDescription; if (normalized === "ALL_PROXY") return props.copy.allProxyDescription; return props.copy.noProxyDescription; }
function variableActionLabel(name: string, action: "write" | "about"): string { return (action === "write" ? props.copy.writeVariable : props.copy.aboutVariable).replace("{name}", name); }
function isManagedVariableSelected(name: string): boolean { const key = managedVariableKey(name); return key !== undefined && props.selectedVariables.includes(key); }
function isLastManagedVariable(name: string): boolean { return isManagedVariableSelected(name) && props.selectedVariables.length === 1; }
</script>
<template>
    <section class="variables-section" :class="{ 'is-readonly': !editable }" :aria-label="copy.localVariables">
      <div v-if="editable" class="local-variable-heading"><span>{{ copy.localVariables }}</span><span>{{ copy.localSyncScope }}</span></div>
      <div v-if="environment.entries.length" class="variable-table">
        <div v-for="entry in visibleEntries" :key="entry.name" class="variable-row">
          <div class="variable-label"><code class="variable-name">{{ entry.name }}</code><HelpTooltip :label="variableActionLabel(entry.name, 'about')" :text="variableDescription(entry.name)" pinnable /></div>
          <details v-if="!editable && entry.value && entry.value.length > 120" class="variable-value-details"><summary><code>{{ entry.value.slice(0, 100) }}…</code><span class="collapsed-label">{{ copy.localExpandValue }}</span><span class="expanded-label">{{ copy.localCollapseValue }}</span></summary><code>{{ entry.value }}</code></details>
          <code v-else class="variable-value" :class="{ unset: !entry.exists }">{{ entry.value ?? copy.unset }}</code>
          <label v-if="editable && managedVariableKey(entry.name)" class="variable-choice" :class="{ locked: isLastManagedVariable(entry.name) }"><input type="checkbox" :checked="isManagedVariableSelected(entry.name)" :disabled="toggling || isLastManagedVariable(entry.name)" :aria-label="variableActionLabel(entry.name, 'write')" @change="emit('toggleVariable', entry.name)" /><span aria-hidden="true"><svg viewBox="0 0 16 16"><path d="m4 8.2 2.5 2.5L12 5.5"/></svg></span></label><span v-else-if="editable" class="variable-choice-spacer"></span>
        </div>
      </div>
      <div v-else class="skeleton-list" :aria-label="copy.detecting"><span v-for="item in 4" :key="item"></span></div>
    </section>


</template>
