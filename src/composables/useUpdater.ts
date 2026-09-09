import { computed, effectScope, ref, shallowRef, watch } from "vue";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { t } from "../i18n";
import { createUpdaterSync, type UpdateCommand, type UpdateInfo, type UpdateSnapshot } from "../lib/updaterSync";

export type UpdateStatus = "idle" | "checking" | "noUpdate" | "available" | "downloading" | "ready" | "error";
export type UpdateErrorPhase = "check" | "install" | "relaunch";

type CheckForUpdateOptions = {
  silent?: boolean;
  openDialog?: boolean;
};

const isTauri = "__TAURI_INTERNALS__" in window;
const UPDATE_CHECK_TIMEOUT_MS = 8_000;
const updaterScope = effectScope(true);
let sharedUpdater: ReturnType<typeof createUpdater> | undefined;

export function useUpdater() {
  return sharedUpdater ??= updaterScope.run(createUpdater)!;
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    sharedUpdater?.dispose();
    updaterScope.stop();
  });
}

function createUpdater() {
  const windowLabel = isTauri ? getCurrentWindow().label : "main";
  const updateStatus = ref<UpdateStatus>("idle");
  const availableUpdate = shallowRef<UpdateInfo | null>(null);
  let pendingUpdate: Update | null = null;
  const updateError = ref<string | null>(null);
  const updateErrorPhase = ref<UpdateErrorPhase>("check");
  const updateDialogOpen = ref(false);
  const updateDownloadedBytes = ref(0);
  const updateTotalBytes = ref<number | null>(null);
  const pendingCheckDialogs = new Set<string>();

  const hasAvailableUpdate = computed(() =>
    Boolean(availableUpdate.value && ["available", "downloading", "ready", "error"].includes(updateStatus.value)),
  );

  const updateProgressPercent = computed(() => {
    if (updateStatus.value === "ready") return 100;
    if (!updateTotalBytes.value || updateTotalBytes.value <= 0) return null;
    return Math.min(100, Math.max(0, Math.round(updateDownloadedBytes.value / updateTotalBytes.value * 100)));
  });

  const isUpdateBusy = computed(() => updateStatus.value === "checking" || updateStatus.value === "downloading");

  const updateButtonText = computed(() => {
    if (updateStatus.value === "checking") return t("update.button.checking");
    if (updateStatus.value === "downloading") {
      const percent = updateProgressPercent.value;
      return `${t("update.button.downloading")}${percent === null ? "" : ` ${percent}%`}`;
    }
    if (updateStatus.value === "ready") return t("update.button.ready");
    if (updateStatus.value === "available") return t("update.button.view");
    if (updateStatus.value === "error" && availableUpdate.value) return t("update.button.view");
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

  const sync = isTauri ? createUpdaterSync({
    windowLabel,
    getSnapshot,
    onSnapshot: applySnapshot,
    onCommand: runCommand,
  }) : null;

  watch([updateStatus, availableUpdate, updateError, updateErrorPhase], () => sync?.publish());
  watch([updateDownloadedBytes, updateTotalBytes], () => sync?.scheduleProgress());

  function getSnapshot(): UpdateSnapshot {
    return {
      status: updateStatus.value,
      update: availableUpdate.value,
      error: updateError.value,
      errorPhase: updateErrorPhase.value,
      downloadedBytes: updateDownloadedBytes.value,
      totalBytes: updateTotalBytes.value,
    };
  }

  function applySnapshot(snapshot: UpdateSnapshot, openDialogFor?: string) {
    updateStatus.value = snapshot.status;
    availableUpdate.value = snapshot.update;
    updateError.value = snapshot.error;
    updateErrorPhase.value = snapshot.errorPhase;
    updateDownloadedBytes.value = snapshot.downloadedBytes;
    updateTotalBytes.value = snapshot.totalBytes;
    if (openDialogFor === windowLabel) updateDialogOpen.value = true;
  }

  function showDialogFor(source: string) {
    if (source === windowLabel) updateDialogOpen.value = true;
    sync?.publish(source);
  }

  async function runCommand(command: UpdateCommand) {
    if (command.action === "check") {
      const wantsDialog = !command.options?.silent || command.options.openDialog;
      if (updateStatus.value === "downloading" || updateStatus.value === "ready") {
        if (wantsDialog) showDialogFor(command.source);
        return;
      }
      if (wantsDialog) pendingCheckDialogs.add(command.source);
      if (updateStatus.value === "checking") return;
      await executeCheck(command.options);
      if (["available", "error"].includes(updateStatus.value)) {
        pendingCheckDialogs.forEach(showDialogFor);
      }
      pendingCheckDialogs.clear();
    } else if (command.action === "install") {
      if (isUpdateBusy.value || updateStatus.value === "ready") return;
      await executeInstall();
      if (updateStatus.value === "error") showDialogFor(command.source);
    } else if (command.action === "relaunch") {
      if (updateStatus.value !== "ready" && updateErrorPhase.value !== "relaunch") return;
      await executeRelaunch();
      if (updateStatus.value === "error") showDialogFor(command.source);
    }
  }

  async function dispatch(action: UpdateCommand["action"], options?: CheckForUpdateOptions) {
    try {
      await sync?.ready;
      const command = { action, source: windowLabel, options };
      if (windowLabel === "main") await runCommand(command);
      else await sync?.sendCommand(command);
    } catch (error) {
      updateStatus.value = "error";
      updateErrorPhase.value = action === "install" ? "install" : action === "relaunch" ? "relaunch" : "check";
      updateError.value = normalizeUpdateError(error, updateErrorPhase.value);
      if (!options?.silent) updateDialogOpen.value = true;
    }
  }

  async function checkForUpdate(options: CheckForUpdateOptions = {}) {
    await dispatch("check", options);
  }

  async function executeCheck(options: CheckForUpdateOptions = {}) {
    const previousStatus = updateStatus.value;
    const previousUpdate = availableUpdate.value;
    updateStatus.value = "checking";
    updateError.value = null;
    updateErrorPhase.value = "check";
    resetUpdateProgress();

    try {
      if (!isTauri) {
        throw new Error(t("update.error.desktopOnly"));
      }

      const nextUpdate = await check({ timeout: UPDATE_CHECK_TIMEOUT_MS });
      const previousResource = pendingUpdate;
      pendingUpdate = nextUpdate;
      if (previousResource) {
        void previousResource.close().catch((error) => console.warn("[ipaste] failed to release update resource", error));
      }
      if (!nextUpdate) {
        availableUpdate.value = null;
        updateStatus.value = options.silent ? "idle" : "noUpdate";
        return;
      }

      availableUpdate.value = { currentVersion: nextUpdate.currentVersion, version: nextUpdate.version, body: nextUpdate.body };
      updateStatus.value = "available";
    } catch (unknownError) {
      console.warn("[ipaste] update check failed", unknownError);
      if (options.silent && pendingCheckDialogs.size === 0) {
        availableUpdate.value = previousUpdate;
        updateStatus.value = previousStatus === "available" && previousUpdate ? "available" : "idle";
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
    await dispatch("install");
  }

  async function executeInstall() {
    if (!pendingUpdate) return;

    updateStatus.value = "downloading";
    updateError.value = null;
    updateErrorPhase.value = "install";
    resetUpdateProgress();

    try {
      await pendingUpdate.downloadAndInstall((event) => {
        handleDownloadEvent(event);
      });
      updateStatus.value = "ready";
    } catch (unknownError) {
      console.warn("[ipaste] update install failed", unknownError);
      updateStatus.value = "error";
      updateErrorPhase.value = "install";
      updateError.value = normalizeUpdateError(unknownError, "install");
    }
  }

  async function relaunchForUpdate() {
    await dispatch("relaunch");
  }

  async function executeRelaunch() {
    try {
      await relaunch();
    } catch (unknownError) {
      console.warn("[ipaste] update relaunch failed", unknownError);
      updateStatus.value = "error";
      updateErrorPhase.value = "relaunch";
      updateError.value = normalizeUpdateError(unknownError, "relaunch");
    }
  }

  function dismissUpdateDialog() {
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
    updateProgressPercent,
    hasAvailableUpdate,
    isUpdateBusy,
    updateButtonText,
    updateSummaryText,
    checkForUpdate,
    openUpdateDialog,
    installAvailableUpdate,
    relaunchForUpdate,
    dismissUpdateDialog,
    dispose: () => sync?.dispose(),
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
