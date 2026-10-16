<script setup lang="ts">
import { onBeforeUnmount, ref } from "vue";
import type { Copy } from "../../../shared/i18n";
defineProps<{ copy: Copy }>();
const dialog = ref<HTMLDialogElement>();
let resolve: ((accepted: boolean) => void) | undefined;
function close(accepted = false) {
  dialog.value?.close();
  resolve?.(accepted);
  resolve = undefined;
}
function open(): Promise<boolean> {
  if (resolve || !dialog.value) return Promise.resolve(false);
  return new Promise<boolean>(done => { resolve = done; dialog.value!.showModal(); });
}
onBeforeUnmount(() => close());
defineExpose({ open });
</script>
<template>
  <dialog ref="dialog" class="confirmation-dialog update-install-dialog" aria-labelledby="update-install-title" aria-describedby="update-install-description" @cancel.prevent="close()">
    <form @submit.prevent="close(true)">
      <h2 id="update-install-title">{{ copy.updateConfirmTitle }}</h2>
      <p id="update-install-description">{{ copy.updateConfirmBody }}</p>
      <div class="dialog-actions">
        <button class="secondary-action" type="button" autofocus @click="close()">{{ copy.updateLater }}</button>
        <button class="primary-action" type="submit">{{ copy.updateConfirmInstall }}</button>
      </div>
    </form>
  </dialog>
</template>
<style scoped>
.update-install-dialog { width:min(440px, calc(100vw - 32px)); }
.dialog-actions { display:flex; justify-content:flex-end; flex-wrap:wrap; gap:10px; margin-top:24px; }
</style>
