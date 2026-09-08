import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { computed, onMounted, onUnmounted, ref } from "vue";
import { ipasteApi } from "../lib/ipasteApi";
import { useIpasteStore } from "../stores/ipasteStore";
import type { OcrInstallProgress, OcrInstallStatus, OcrMode } from "../types";

export const OCR_MODELS = [
  { mode: "fast", name: "PP-OCRv6 Tiny", totalBytes: 6_932_119 },
  { mode: "best", name: "PP-OCRv6 Small", totalBytes: 31_824_456 },
] as const;

export function formatOcrBytes(bytes: number) {
  return `${(Math.max(0, bytes) / 1_000_000).toFixed(1)} MB`;
}

type Operation = { mode: OcrMode; kind: "install" | "repair" | "switch" };
type Failure = Operation & { detail: string };

export function useOcrModels() {
  const store = useIpasteStore();
  const statuses = ref<Record<OcrMode, OcrInstallStatus | null>>({ fast: null, best: null });
  const loading = ref(true);
  const removing = ref(false);
  const operation = ref<Operation | null>(null);
  const progress = ref<OcrInstallProgress | null>(null);
  const failure = ref<Failure | null>(null);
  const error = ref<string | null>(null);
  const notice = ref<"repaired" | "removed" | null>(null);
  const bytesPerSecond = ref(0);
  let unlisten: UnlistenFn | undefined;
  let speedTimer: ReturnType<typeof setInterval> | undefined;
  let disposed = false;
  let speedSample = { bytes: 0, time: 0 };

  const busy = computed(() => loading.value || removing.value || operation.value !== null);
  const hasModels = computed(() => Object.values(statuses.value).some((status) => status?.hasResources || (status?.downloadedBytes ?? 0) > 0));
  const installDir = computed(() => statuses.value.fast?.installDir || statuses.value.best?.installDir || "");
  const percent = computed(() => {
    if (!progress.value?.totalBytes) return 0;
    return Math.min(100, Math.floor(progress.value.downloadedBytes / progress.value.totalBytes * 100));
  });

  function installed(mode: OcrMode) {
    return Boolean(statuses.value[mode]?.installed);
  }

  function inUse(mode: OcrMode) {
    return installed(mode) && store.ocrMode === mode;
  }

  function needsRepair(mode: OcrMode) {
    return Boolean(statuses.value[mode]?.needsRepair);
  }

  async function refresh() {
    const [fast, best] = await Promise.all([
      ipasteApi.ocrInstallStatus("fast"),
      ipasteApi.ocrInstallStatus("best"),
    ]);
    statuses.value = { fast, best };
  }

  async function load() {
    loading.value = true;
    error.value = null;
    try {
      await refresh();
    } catch (cause) {
      error.value = String(cause);
    } finally {
      loading.value = false;
    }
  }

  function stopSpeedTimer() {
    clearInterval(speedTimer);
    speedTimer = undefined;
    bytesPerSecond.value = 0;
  }

  function startSpeedTimer() {
    stopSpeedTimer();
    speedSample = { bytes: 0, time: performance.now() };
    speedTimer = setInterval(() => {
      const time = performance.now();
      const bytes = progress.value?.networkBytes ?? 0;
      bytesPerSecond.value = progress.value?.phase === "downloading"
        ? Math.max(0, bytes - speedSample.bytes) * 1000 / Math.max(1, time - speedSample.time)
        : 0;
      speedSample = { bytes, time };
    }, 1000);
  }

  async function run(mode: OcrMode, kind: Operation["kind"]) {
    if (busy.value) return;
    operation.value = { mode, kind };
    failure.value = null;
    error.value = null;
    notice.value = null;
    try {
      if (kind === "switch") {
        await store.updateOcrMode(mode);
      } else {
        progress.value = {
          mode, phase: "fetchingManifest", fileName: null,
          downloadedBytes: 0, networkBytes: 0,
          totalBytes: OCR_MODELS.find((model) => model.mode === mode)!.totalBytes,
        };
        startSpeedTimer();
        const result = await ipasteApi.installOcrAssets(mode, kind === "install");
        statuses.value[mode] = result;
        // Activation happens in Rust only after verification, even if this window closes.
        if (kind === "install") await store.loadSettings();
        else notice.value = "repaired";
      }
      await refresh();
    } catch (cause) {
      failure.value = { mode, kind, detail: String(cause) };
      // Keep successfully downloaded files available after a partial failure.
      await refresh().catch(() => {});
    } finally {
      stopSpeedTimer();
      operation.value = null;
      progress.value = null;
    }
  }

  function choose(mode: OcrMode) {
    // Keep an interrupted download's activation intent; a failed switch rechecks file state.
    if (failure.value?.mode === mode && failure.value.kind !== "switch") {
      return run(mode, failure.value.kind);
    }
    return run(mode, installed(mode) ? "switch" : needsRepair(mode) ? "repair" : "install");
  }

  async function removeAll() {
    if (busy.value) return;
    removing.value = true;
    error.value = null;
    failure.value = null;
    notice.value = null;
    try {
      await ipasteApi.removeOcrAssets();
      await refresh();
      notice.value = "removed";
    } catch (cause) {
      error.value = String(cause);
    } finally {
      removing.value = false;
    }
  }

  onMounted(async () => {
    try {
      if ("__TAURI_INTERNALS__" in window) {
        const cleanup = await listen<OcrInstallProgress>("ipaste://ocr-install-progress", ({ payload }) => {
          if (payload.mode === operation.value?.mode) progress.value = payload;
        });
        if (disposed) { cleanup(); return; }
        unlisten = cleanup;
      }
      await load();
    } catch (cause) {
      error.value = String(cause);
      loading.value = false;
    }
  });

  onUnmounted(() => {
    disposed = true;
    unlisten?.();
    stopSpeedTimer();
  });

  return {
    statuses, loading, removing, operation, progress, failure, error, notice,
    bytesPerSecond, busy, hasModels, installDir, percent,
    installed, inUse, needsRepair, load, choose, removeAll,
  };
}
