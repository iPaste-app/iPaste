import { emit, emitTo, listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { UpdateErrorPhase, UpdateStatus } from "../composables/useUpdater";

export type UpdateInfo = { currentVersion: string; version: string; body?: string };
export type UpdateSnapshot = {
  status: UpdateStatus;
  update: UpdateInfo | null;
  error: string | null;
  errorPhase: UpdateErrorPhase;
  downloadedBytes: number;
  totalBytes: number | null;
};
export type UpdateCommand = {
  action: "sync" | "check" | "install" | "relaunch";
  source: string;
  options?: { silent?: boolean; openDialog?: boolean };
};

const COMMAND_EVENT = "ipaste://updater-command";
const STATE_EVENT = "ipaste://updater-state";

// The main window owns the native updater resource and download channel, so
// closing the settings window never interrupts a download or loses its progress.
export function createUpdaterSync(options: {
  windowLabel: string;
  getSnapshot: () => UpdateSnapshot;
  onSnapshot: (snapshot: UpdateSnapshot, openDialogFor?: string) => void;
  onCommand: (command: UpdateCommand) => Promise<void>;
}) {
  const isMain = options.windowLabel === "main";
  const unlisteners: UnlistenFn[] = [];
  let disposed = false;
  let progressTimer: ReturnType<typeof setTimeout> | undefined;

  async function register(pending: Promise<UnlistenFn>) {
    const unlisten = await pending;
    if (disposed) unlisten();
    else unlisteners.push(unlisten);
  }

  const ready = (async () => {
    if (isMain) {
      await register(listen<UpdateCommand>(COMMAND_EVENT, ({ payload }) => {
        if (payload.action === "sync") publish();
        else void options.onCommand(payload).catch(reportError);
      }));
      if (!disposed) publish();
    } else {
      await register(listen<{ snapshot: UpdateSnapshot; openDialogFor?: string }>(STATE_EVENT, ({ payload }) => {
        options.onSnapshot(payload.snapshot, payload.openDialogFor);
      }));
      if (!disposed) await emitTo("main", COMMAND_EVENT, { action: "sync", source: options.windowLabel });
    }
  })();
  void ready.catch(reportError);

  function publish(openDialogFor?: string) {
    if (!isMain || disposed) return;
    clearTimeout(progressTimer);
    progressTimer = undefined;
    void emit(STATE_EVENT, { snapshot: options.getSnapshot(), openDialogFor }).catch(reportError);
  }

  function scheduleProgress() {
    if (!isMain || disposed || progressTimer !== undefined) return;
    progressTimer = setTimeout(() => publish(), 100);
  }

  async function sendCommand(command: UpdateCommand) {
    await ready;
    await emitTo("main", COMMAND_EVENT, command);
  }

  function dispose() {
    disposed = true;
    clearTimeout(progressTimer);
    unlisteners.forEach((unlisten) => unlisten());
  }

  return { ready, publish, scheduleProgress, sendCommand, dispose };
}

function reportError(error: unknown) {
  console.warn("[ipaste] updater window sync failed", error);
}
