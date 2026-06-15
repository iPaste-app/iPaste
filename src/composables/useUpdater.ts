import { computed, ref, shallowRef } from "vue";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { t } from "../i18n";

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
    if (updateStatus.value === "checking") return t("update.button.checking");
    if (updateStatus.value === "downloading") return t("update.button.downloading");
    if (updateStatus.value === "ready") return t("update.button.ready");
    return t("update.button.idle");
  });

  const updateSummaryText = computed(() => {
    if (updateStatus.value === "checking") return t("update.summary.checking");
    if (updateStatus.value === "noUpdate") return t("update.summary.noUpdate");
    if (updateStatus.value === "available" && availableUpdate.value) {
      return t("update.summary.available", { version: availableUpdate.value.version });
    }
    if (updateStatus.value === "downloading") return t("update.summary.downloading");
    if (updateStatus.value === "ready") return t("update.summary.ready");
    if (updateStatus.value === "error") return updateError.value ?? getFallbackErrorText(updateErrorPhase.value);
    return t("update.summary.idle");
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
        throw new Error(t("update.error.desktopOnly"));
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

  if (normalized.includes("请在桌面应用中检查更新") || normalized.includes("check for updates in the desktop app")) {
    return t("update.error.desktopOnly");
  }

  if (normalized.includes("signature") || normalized.includes("verify")) {
    return t("update.error.signature");
  }

  if (normalized.includes("invalid updater binary format") || normalized.includes("binary for the current target")) {
    return t("update.error.invalidPackage");
  }

  if (normalized.includes("permission denied") || normalized.includes("access is denied")) {
    return phase === "install"
      ? t("update.error.installPermission")
      : t("update.error.permission");
  }

  if (normalized.includes("failed to install") || normalized.includes("packageinstallfailed")) {
    return t("update.error.installFailed");
  }

  if (normalized.includes("404") || normalized.includes("not found")) {
    return t("update.error.notFound");
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
      ? t("update.error.downloadNetwork")
      : t("update.error.checkNetwork");
  }

  return getFallbackErrorText(phase);
}

function getFallbackErrorText(phase: UpdateErrorPhase) {
  if (phase === "install") return t("update.error.installFallback");
  if (phase === "relaunch") return t("update.error.relaunchFallback");
  return t("update.error.checkFallback");
}

export function cleanUpdateNotes(notes: string | undefined | null) {
  const trimmed = notes?.trim() ?? "";
  if (/^Built from .+ commit [0-9a-f]{7,40}\.?$/i.test(trimmed)) {
    return "";
  }

  return trimmed;
}
