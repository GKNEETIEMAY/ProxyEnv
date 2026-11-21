import { withoutWindowsExtendedPathPrefix } from "../../shared/utils/path";
import type { RemoteTarget } from "./state";

const targetCopiesStorageKey = "proxyenv.remoteBridge.targetCopies";
type TargetCopyStorage = Pick<Storage, "getItem" | "setItem">;

export function readTargetCopies(getStorage: () => TargetCopyStorage): { hidden: string[]; names: Record<string, string> } {
  try {
    const parsed = JSON.parse(getStorage().getItem(targetCopiesStorageKey) ?? "null");
    return {
      hidden: Array.isArray(parsed?.hidden) ? parsed.hidden.filter((value: unknown): value is string => typeof value === "string") : [],
      names: parsed?.names && typeof parsed.names === "object" ? parsed.names : {},
    };
  } catch {
    return { hidden: [], names: {} };
  }
}

export function writeTargetCopies(
  getStorage: () => TargetCopyStorage,
  hidden: ReadonlySet<string>,
  names: ReadonlyMap<string, string>,
) {
  try {
    getStorage().setItem(targetCopiesStorageKey, JSON.stringify({
      hidden: [...hidden],
      names: Object.fromEntries(names),
    }));
  } catch {
    // Display copies remain usable for the current session when storage is unavailable.
  }
}

export function applyTargetCopies(
  targets: readonly RemoteTarget[],
  hidden: ReadonlySet<string>,
  names: ReadonlyMap<string, string>,
): RemoteTarget[] {
  return targets
    .filter((target) => !hidden.has(target.id))
    .map((target) => ({
      ...target,
      configPath: withoutWindowsExtendedPathPrefix(target.configPath),
      displayName: target.source === "manual" ? target.displayName : names.get(target.id) ?? target.displayName,
    }));
}
