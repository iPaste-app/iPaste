import { computed, ref, shallowRef } from "vue";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdateStatus = "idle" | "checking" | "noUpdate" | "available" | "downloading" | "ready" | "error";
export type UpdateErrorPhase = "check" | "install" | "relaunch";

type CheckForUpdateOptions = {
  silent?: boolean;
  openDialog?: boolean;
};

const isTauri = "__TAURI_INTERNALS__" in window;
const UPDATE_CHECK_TIMEOUT_MS = 8_000;

export function useUpdater() {
  const updateStatus = ref<UpdateStatus>("idle");
  const availableUpdate = shallowRef<Update | null>(null);
  const updateError = ref<string | null>(null);
  const updateErrorPhase = ref<UpdateErrorPhase>("check");
  const updateDialogOpen = ref(false);
  const updateDownloadedBytes = ref(0);
  const updateTotalBytes = ref<number | null>(null);

  const hasAvailableUpdate = computed(() =>
    Boolean(availableUpdate.value && ["available", "downloading", "ready"].includes(updateStatus.value)),
  );

  const isUpdateBusy = computed(() => updateStatus.value === "checking" || updateStatus.value === "downloading");

  const updateButtonText = computed(() => {
    if (updateStatus.value === "checking") return "检查中";
    if (updateStatus.value === "downloading") return "更新中";
    if (updateStatus.value === "ready") return "等待重启";
    return "检查更新";
  });

  const updateSummaryText = computed(() => {
    if (updateStatus.value === "checking") return "正在检查更新。";
    if (updateStatus.value === "noUpdate") return "已是最新版。";
    if (updateStatus.value === "available" && availableUpdate.value) {
      return `新版本 ${availableUpdate.value.version} 可用。`;
    }
    if (updateStatus.value === "downloading") return "正在更新。";
    if (updateStatus.value === "ready") return "更新已安装，重启后生效。";
    if (updateStatus.value === "error") return updateError.value ?? getFallbackErrorText(updateErrorPhase.value);
    return "检查是否有可用更新。";
  });

  async function checkForUpdate(options: CheckForUpdateOptions = {}) {
    if (isUpdateBusy.value) return;

    const previousStatus = updateStatus.value;
    updateStatus.value = "checking";
    updateError.value = null;
    updateErrorPhase.value = "check";
    availableUpdate.value = null;
    updateDialogOpen.value = false;
    resetUpdateProgress();

    try {
      if (!isTauri) {
        throw new Error("请在桌面应用中检查更新。");
      }

      const nextUpdate = await check({ timeout: UPDATE_CHECK_TIMEOUT_MS });
      if (!nextUpdate) {
        updateStatus.value = options.silent ? "idle" : "noUpdate";
        return;
      }

      availableUpdate.value = nextUpdate;
      updateStatus.value = "available";
      updateDialogOpen.value = !options.silent || Boolean(options.openDialog);
    } catch (unknownError) {
      console.warn("[ipaste] update check failed", unknownError);
      if (options.silent) {
        updateStatus.value = previousStatus === "available" && availableUpdate.value ? "available" : "idle";
        return;
      }

      updateStatus.value = "error";
      updateErrorPhase.value = "check";
      updateError.value = normalizeUpdateError(unknownError, "check");
    }
  }

  function openUpdateDialog() {
    if (availableUpdate.value) {
      updateDialogOpen.value = true;
      return;
    }

    void checkForUpdate({ openDialog: true });
  }

  async function installAvailableUpdate() {
    if (!availableUpdate.value || updateStatus.value === "downloading") return;

    updateStatus.value = "downloading";
    updateError.value = null;
    updateErrorPhase.value = "install";
    resetUpdateProgress();

    try {
      await availableUpdate.value.downloadAndInstall((event) => {
        handleDownloadEvent(event);
      });
      updateStatus.value = "ready";
    } catch (unknownError) {
      console.warn("[ipaste] update install failed", unknownError);
      updateStatus.value = "error";
      updateErrorPhase.value = "install";
      updateDialogOpen.value = true;
      updateError.value = normalizeUpdateError(unknownError, "install");
    }
  }

  async function relaunchForUpdate() {
    try {
      await relaunch();
    } catch (unknownError) {
      console.warn("[ipaste] update relaunch failed", unknownError);
      updateStatus.value = "error";
      updateErrorPhase.value = "relaunch";
      updateDialogOpen.value = true;
      updateError.value = normalizeUpdateError(unknownError, "relaunch");
    }
  }

  function dismissUpdateDialog() {
    if (updateStatus.value === "downloading") return;
    updateDialogOpen.value = false;
  }

  function resetUpdateProgress() {
    updateDownloadedBytes.value = 0;
    updateTotalBytes.value = null;
  }

  function handleDownloadEvent(event: DownloadEvent) {
    if (event.event === "Started") {
      updateTotalBytes.value = event.data.contentLength ?? null;
      updateDownloadedBytes.value = 0;
      return;
    }

    if (event.event === "Progress") {
      updateDownloadedBytes.value += event.data.chunkLength;
      return;
    }

    if (event.event === "Finished" && updateTotalBytes.value) {
      updateDownloadedBytes.value = updateTotalBytes.value;
    }
  }

  return {
    updateStatus,
    availableUpdate,
    updateError,
    updateErrorPhase,
    updateDialogOpen,
    updateDownloadedBytes,
    updateTotalBytes,
    hasAvailableUpdate,
    isUpdateBusy,
    updateButtonText,
    updateSummaryText,
    checkForUpdate,
    openUpdateDialog,
    installAvailableUpdate,
    relaunchForUpdate,
    dismissUpdateDialog,
  };
}

export function normalizeUpdateError(error: unknown, phase: UpdateErrorPhase = "check") {
  const message = error instanceof Error ? error.message : String(error);
  const normalized = message.toLowerCase();

  if (normalized.includes("请在桌面应用中检查更新")) {
    return "请在桌面应用中检查更新。";
  }

  if (normalized.includes("signature") || normalized.includes("verify")) {
    return "更新包校验失败，请稍后重试或重新下载安装包。";
  }

  if (normalized.includes("invalid updater binary format") || normalized.includes("binary for the current target")) {
    return "更新包格式不正确，请重新发布安装包后重试。";
  }

  if (normalized.includes("permission denied") || normalized.includes("access is denied")) {
    return phase === "install"
      ? "无法写入或启动更新安装包，请退出 iPaste 后重新尝试。"
      : "权限不足，请退出 iPaste 后重新尝试。";
  }

  if (normalized.includes("failed to install") || normalized.includes("packageinstallfailed")) {
    return "更新包下载完成，但安装失败。请退出 iPaste 后重新启动应用再试。";
  }

  if (normalized.includes("404") || normalized.includes("not found")) {
    return "暂时无法获取更新信息，请稍后重试。";
  }

  if (
    normalized.includes("timed out") ||
    normalized.includes("timeout") ||
    normalized.includes("network") ||
    normalized.includes("request") ||
    normalized.includes("dns") ||
    normalized.includes("failed to fetch") ||
    normalized.includes("error sending request")
  ) {
    return phase === "install"
      ? "暂时无法下载更新包，请确认网络连接后重试。"
      : "暂时无法检查更新，请确认网络连接后重试。";
  }

  return getFallbackErrorText(phase);
}

function getFallbackErrorText(phase: UpdateErrorPhase) {
  if (phase === "install") return "无法下载或安装更新，请稍后重试。";
  if (phase === "relaunch") return "更新已安装，但无法自动重启。请手动重启 iPaste。";
  return "无法检查更新，请稍后重试。";
}

export function cleanUpdateNotes(notes: string | undefined | null) {
  const trimmed = notes?.trim() ?? "";
  if (/^Built from .+ commit [0-9a-f]{7,40}\.?$/i.test(trimmed)) {
    return "";
  }

  return trimmed;
}
