import { computed, onBeforeUnmount, ref, watch, type Ref } from "vue";
import type { BridgeSummary, RemoteTarget, TargetPlatformProbe } from "../state";

export function useRemoteTargetPlatform(
  target: Readonly<Ref<RemoteTarget | undefined>>,
  summary: () => BridgeSummary,
  probe: (id: string) => Promise<TargetPlatformProbe>,
) {
  const observation = ref<TargetPlatformProbe>();
  const pending = ref(false);
  const probeError = ref<unknown>();
  let revision = 0;

  function cancelProbe() {
    revision++;
    pending.value = false;
  }
  watch(target, () => {
    cancelProbe();
    observation.value = undefined;
    probeError.value = undefined;
  }, { flush: "sync" });
  onBeforeUnmount(cancelProbe);

  const currentSummary = computed(() => {
    const current = summary();
    return target.value && current.target?.id === target.value.id ? current : undefined;
  });
  const platform = computed(() => {
    if (!target.value?.available) return "unsupported";
    return currentSummary.value?.targetPlatform ?? observation.value?.platform ?? "unknown";
  });
  const windowsBridge = computed(() => platform.value === "windows"
    ? currentSummary.value?.windowsBridge ?? observation.value?.windowsBridge
    : undefined);

  async function checkPlatform() {
    const id = target.value?.id;
    if (!id || pending.value || !target.value?.available) return;
    const ticket = ++revision;
    pending.value = true;
    probeError.value = undefined;
    try {
      const result = await probe(id);
      if (ticket === revision && target.value?.id === result.targetId) observation.value = result;
    } catch (cause) {
      if (ticket === revision) probeError.value = cause;
    } finally {
      if (ticket === revision) pending.value = false;
    }
  }

  return { platform, windowsBridge, checkPlatform, cancelProbe, pending, probeError };
}
