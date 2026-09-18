import { invoke } from "@tauri-apps/api/core";
import { onMounted, onUnmounted, ref } from "vue";

type PanelSize = { width: number; height: number };

/** Keep sizes in logical pixels so restoring on another display respects its scale. */
export function usePanelViewport(enabled: boolean, onResize: () => void) {
  const width = ref(window.innerWidth);
  let timer: number | undefined;
  let pendingSize: PanelSize | null = null;
  let saving = Promise.resolve();

  function flushSize() {
    window.clearTimeout(timer);
    timer = undefined;
    const size = pendingSize;
    pendingSize = null;
    if (size && "__TAURI_INTERNALS__" in window) {
      // Serialize writes so a slower earlier resize cannot overwrite the final size.
      saving = saving.then(() => invoke<void>("save_main_window_size", { size }))
        .catch(error => {
          console.error("Failed to save panel size", error);
        });
    }
    return saving;
  }

  function handleResize() {
    width.value = window.innerWidth;
    onResize();
    if (width.value < 280 || width.value > 720 || window.innerHeight < 500) return;
    pendingSize = { width: width.value, height: window.innerHeight };
    window.clearTimeout(timer);
    timer = window.setTimeout(() => void flushSize(), 200);
  }

  function handleBlur() {
    void flushSize();
  }

  onMounted(() => {
    if (!enabled) return;
    window.addEventListener("resize", handleResize);
    window.addEventListener("blur", handleBlur);
  });

  onUnmounted(() => {
    if (!enabled) return;
    window.removeEventListener("resize", handleResize);
    window.removeEventListener("blur", handleBlur);
    void flushSize();
  });

  return { width, flushSize };
}
