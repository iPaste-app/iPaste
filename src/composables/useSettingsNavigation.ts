import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { onMounted, onUnmounted } from "vue";
import { ipasteApi } from "../lib/ipasteApi";
import type { SettingsTarget } from "../types";

// Listen before consuming the queued target so requests during window startup are not lost.
export function useSettingsNavigation(openTarget: (target: SettingsTarget) => void, onError: (error: unknown) => void) {
  let disposed = false;
  let unlisten: UnlistenFn | undefined;

  async function consumeTarget() {
    try {
      const target = await ipasteApi.takeSettingsTarget();
      if (!disposed && target) openTarget(target);
    } catch (error) {
      if (!disposed) onError(error);
    }
  }

  onMounted(async () => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    try {
      const cleanup = await listen("ipaste://settings-navigation", () => { void consumeTarget(); });
      if (disposed) { cleanup(); return; }
      unlisten = cleanup;
      await consumeTarget();
    } catch (error) {
      if (!disposed) onError(error);
    }
  });

  onUnmounted(() => { disposed = true; unlisten?.(); });
}
