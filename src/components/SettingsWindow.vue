<script setup lang="ts">
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openPath } from "@tauri-apps/plugin-opener";
import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  AlertCircle,
  AppWindow,
  Blocks,
  Box,
  Brush,
  CheckCircle2,
  ChevronRight,
  Cloud,
  ClipboardPlus,
  Cpu,
  Database,
  Download,
  FolderOpen,
  History,
  Keyboard,
  LoaderCircle,
  RefreshCw,
  RotateCcw,
  Save,
  ScanText,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  SquareCode,
  Tags,
  Unplug,
  Zap,
} from "lucide-vue-next";
import LanguageSelect from "./LanguageSelect.vue";
import UpdateDialog from "./UpdateDialog.vue";
import { useUpdater } from "../composables/useUpdater";
import { languageOptions, t } from "../i18n";
import { ipasteApi } from "../lib/ipasteApi";
import { formatShortcut } from "../lib/format";
import { useIpasteStore } from "../stores/ipasteStore";
import type { AppInfo, Language, OcrInstallProgress, OcrInstallStatus, OcrMode, PanelLayout, PanelOpenBehavior } from "../types";

const store = useIpasteStore();
const DEFAULT_SHORTCUT = "CommandOrControl+Shift+V";
type SettingsTab = "general" | "shortcuts" | "ocr" | "dataManagement" | "permissions" | "about";
const activeTab = ref<SettingsTab>("general");
const showPermissionGuide = ref(false);
const shortcutDraft = ref(DEFAULT_SHORTCUT);
const shortcutRecording = ref(false);
const shortcutMessage = ref<string | null>(null);
const shortcutError = ref<string | null>(null);
const isSavingShortcut = ref(false);
const cloudApiAddress = ref("");
const cloudApiKey = ref("");
const cloudMessage = ref<string | null>(null);
const cloudError = ref<string | null>(null);
const isTestingCloud = ref(false);
const isSavingCloud = ref(false);
const appInfo = ref<AppInfo | null>(null);
const isTauri = "__TAURI_INTERNALS__" in window;
const isMacOs = /mac/i.test(navigator.platform) || /Mac OS/i.test(navigator.userAgent);
const ocrStatus = ref<OcrInstallStatus | null>(null);
const ocrProgress = ref<OcrInstallProgress | null>(null);
const ocrMessage = ref<string | null>(null);
const ocrError = ref<string | null>(null);
const isInstallingOcr = ref(false);
const isRemovingOcr = ref(false);
const lastInstalledOcrMode = ref<OcrMode | null>(null);
let unlistenOcrProgress: UnlistenFn | null = null;
let shouldRestoreAppShortcutAfterRecording = false;
const updater = useUpdater();

const retentionOptions = computed(() => [
  { label: t("settings.retention.7"), value: 7 },
  { label: t("settings.retention.14"), value: 14 },
  { label: t("settings.retention.30"), value: 30 },
  { label: t("settings.retention.90"), value: 90 },
]);

const appendCopyTimeoutOptions = [
  { label: "1", value: 1 },
  { label: "3", value: 3 },
  { label: "5", value: 5 },
  { label: "10", value: 10 },
];

const panelOpenOptions = computed<Array<{ label: string; value: PanelOpenBehavior; icon: typeof History }>>(() => [
  { label: t("settings.panelOpen.history"), value: "history", icon: History },
  { label: t("settings.panelOpen.lastSelected"), value: "last_selected", icon: Tags },
]);

const panelLayoutOptions = computed<Array<{ label: string; value: PanelLayout }>>(() => [
  { label: t("settings.layout.top"), value: "top" },
  { label: t("settings.layout.side"), value: "side" },
]);

const ocrModeOptions = computed<Array<{ label: string; value: OcrMode; description: string; totalBytes: number }>>(() => [
  {
    label: "Fast",
    value: "fast",
    description: t("ocr.mode.fast.description"),
    totalBytes: 37_557_099,
  },
  {
    label: "Best",
    value: "best",
    description: t("ocr.mode.best.description"),
    totalBytes: 59_452_879,
  },
]);

const tabs = computed(() => {
  const items: Array<{ id: SettingsTab; label: string; icon: typeof SlidersHorizontal }> = [
    { id: "general", label: t("settings.tabs.general"), icon: SlidersHorizontal },
    { id: "shortcuts", label: t("settings.tabs.shortcuts"), icon: Keyboard },
    { id: "dataManagement", label: t("settings.tabs.dataManagement"), icon: Database },
    { id: "about", label: t("settings.tabs.about"), icon: Sparkles },
  ];

  if (!isMacOs) {
    items.splice(2, 0, { id: "ocr", label: t("settings.tabs.ocr"), icon: ScanText });
  }

  if (isMacOs) {
    items.splice(3, 0, { id: "permissions", label: t("settings.tabs.permissions"), icon: ShieldCheck });
  }

  return items;
});

const techStack = computed(() => [
  { name: "Vue 3", detail: t("settings.tech.vue"), icon: Blocks, tone: "emerald" },
  { name: "TypeScript", detail: t("settings.tech.ts"), icon: SquareCode, tone: "blue" },
  { name: "Tauri 2", detail: t("settings.tech.tauri"), icon: AppWindow, tone: "teal" },
  { name: "Rust", detail: t("settings.tech.rust"), icon: Cpu, tone: "slate" },
  { name: "Pinia", detail: t("settings.tech.pinia"), icon: Box, tone: "amber" },
  { name: "Tailwind CSS", detail: t("settings.tech.tailwind"), icon: Brush, tone: "sky" },
  { name: "Vite", detail: t("settings.tech.vite"), icon: Zap, tone: "violet" },
  { name: "SQLite", detail: t("settings.tech.sqlite"), icon: Database, tone: "indigo" },
]);

const retentionText = computed(() => {
  return retentionOptions.value.find((option) => option.value === store.retentionDays)?.label ?? t("settings.retention.30");
});

const appendCopyTimeoutText = computed(() => {
  const label = appendCopyTimeoutOptions.find((option) => option.value === store.appendCopyTimeoutMinutes)?.label ?? "1";
  return t("common.minutes", { value: label });
});

const cloudStatusText = computed(() => {
  return store.cloud.enabled ? t("settings.cloud.enabled") : t("settings.cloud.disabled");
});
const selectedOcrModeOption = computed(() => {
  return ocrModeOptions.value.find((option) => option.value === store.ocrMode) ?? ocrModeOptions.value[0];
});
const ocrStatusText = computed(() => {
  if (!ocrStatus.value) return t("ocr.status.checking");
  if (isMacOs) {
    return t("ocr.status.macos");
  }
  if (ocrStatus.value.installed) {
    return t("ocr.status.installed");
  }
  if (lastInstalledOcrMode.value && lastInstalledOcrMode.value !== store.ocrMode) {
    return t("ocr.status.modeNotDownloaded");
  }
  return t("ocr.status.readyToDownload");
});
const ocrDownloadedText = computed(() => {
  const downloaded = ocrProgress.value?.downloadedBytes ?? ocrStatus.value?.downloadedBytes ?? 0;
  const total = ocrProgress.value?.totalBytes ?? ocrStatus.value?.totalBytes ?? selectedOcrModeOption.value.totalBytes;
  return `${formatBytes(downloaded)} / ${formatBytes(total)}`;
});
const ocrInstallPercent = computed(() => {
  const total = ocrProgress.value?.totalBytes ?? ocrStatus.value?.totalBytes ?? 0;
  const downloaded = ocrProgress.value?.downloadedBytes ?? ocrStatus.value?.downloadedBytes ?? 0;
  if (!total) return ocrStatus.value?.installed ? 100 : 0;
  return Math.min(100, Math.round((downloaded / total) * 100));
});
const ocrInstallButtonText = computed(() => {
  if (isInstallingOcr.value) {
    return ocrProgress.value?.phase === "fetchingManifest" ? t("ocr.install.fetchingManifest") : t("ocr.install.downloading");
  }
  if (ocrStatus.value?.installed) {
    return t("ocr.install.repair");
  }
  if (lastInstalledOcrMode.value && lastInstalledOcrMode.value !== store.ocrMode) {
    return t("ocr.install.switchAndDownload");
  }
  return t("ocr.install.download");
});

const formattedShortcutDraft = computed(() => formatShortcut(shortcutDraft.value || store.shortcut));
const canSaveShortcut = computed(() =>
  Boolean(shortcutDraft.value && shortcutDraft.value !== store.shortcut && !isSavingShortcut.value),
);
const fixedShortcuts = computed(() => [
  { keys: [formatShortcut("CommandOrControl+F")], action: t("settings.shortcuts.focusSearch") },
  { keys: ["↑", "↓", "←", "→"], action: t("settings.shortcuts.moveCards") },
  { keys: ["Enter"], action: t("settings.shortcuts.pasteSelected") },
  { keys: ["Esc"], action: t("settings.shortcuts.closePanelOrMenu") },
  { keys: [formatShortcut("CommandOrControl+1")], action: t("settings.shortcuts.switchHistory") },
  { keys: [formatShortcut("CommandOrControl+2")], action: t("settings.shortcuts.switchFirstCategory") },
  { keys: [`${formatShortcut("CommandOrControl+3")} ... ${formatShortcut("CommandOrControl+9")}`], action: t("settings.shortcuts.switchMoreCategories") },
]);

onMounted(async () => {
  await store.load();
  appInfo.value = await ipasteApi.appInfo();
  await loadOcrStatus();
  if (isTauri) {
    unlistenOcrProgress = await listen<OcrInstallProgress>("ipaste://ocr-install-progress", (event) => {
      ocrProgress.value = event.payload;
    });
  }
  resetShortcutForm();
  resetCloudForm();
});

onUnmounted(() => {
  void stopRecordingShortcut({ restoreAppShortcut: true });
  void unlistenOcrProgress?.();
});

async function openAccessibilityGuide() {
  showPermissionGuide.value = true;
  await ipasteApi.openAccessibilitySettings();
}

function resetShortcutForm() {
  shortcutDraft.value = store.shortcut || DEFAULT_SHORTCUT;
  shortcutMessage.value = null;
  shortcutError.value = null;
}

async function startRecordingShortcut() {
  if (shortcutRecording.value) return;
  if (!(await pauseAppShortcutWhileRecording())) return;
  shortcutRecording.value = true;
  shortcutMessage.value = null;
  shortcutError.value = null;
  window.addEventListener("keydown", handleShortcutRecording, { capture: true });
}

async function stopRecordingShortcut(options: { restoreAppShortcut?: boolean } = {}) {
  if (shortcutRecording.value) {
    shortcutRecording.value = false;
    window.removeEventListener("keydown", handleShortcutRecording, { capture: true });
  }
  if (options.restoreAppShortcut) {
    await restoreAppShortcutAfterRecording();
  }
}

function handleShortcutRecording(event: KeyboardEvent) {
  event.preventDefault();
  event.stopPropagation();
  event.stopImmediatePropagation();

  if (event.key === "Escape" && !event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey) {
    void stopRecordingShortcut({ restoreAppShortcut: true });
    return;
  }

  const shortcut = shortcutFromKeyboardEvent(event);
  if (!shortcut) {
    shortcutError.value = t("settings.shortcuts.invalid");
    return;
  }

  shortcutDraft.value = shortcut;
  shortcutError.value = null;
  void stopRecordingShortcut({ restoreAppShortcut: true });
}

async function pauseAppShortcutWhileRecording() {
  if (shouldRestoreAppShortcutAfterRecording) return true;

  try {
    await ipasteApi.setAppShortcutEnabled(false);
    shouldRestoreAppShortcutAfterRecording = true;
    return true;
  } catch (unknownError) {
    shortcutError.value = String(unknownError);
    return false;
  }
}

async function restoreAppShortcutAfterRecording() {
  if (!shouldRestoreAppShortcutAfterRecording) return;
  shouldRestoreAppShortcutAfterRecording = false;

  try {
    await ipasteApi.setAppShortcutEnabled(true);
  } catch (unknownError) {
    shouldRestoreAppShortcutAfterRecording = true;
    shortcutError.value = String(unknownError);
  }
}

function shortcutFromKeyboardEvent(event: KeyboardEvent) {
  const key = shortcutKeyFromEvent(event);
  if (!key) return "";

  const modifiers: string[] = [];
  if (event.metaKey) modifiers.push("Command");
  if (event.ctrlKey) modifiers.push("Control");
  if (event.altKey) modifiers.push("Alt");
  if (event.shiftKey) modifiers.push("Shift");

  if (!modifiers.length) return "";
  return [...modifiers, key].join("+");
}

function shortcutKeyFromEvent(event: KeyboardEvent) {
  const modifierKeys = new Set(["Shift", "Control", "Alt", "Meta", "Command"]);
  if (modifierKeys.has(event.key)) return "";

  if (/^Key[A-Z]$/.test(event.code)) return event.code.slice(3);
  if (/^Digit[0-9]$/.test(event.code)) return event.code.slice(5);
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(event.code)) return event.code;

  const specialKeys: Record<string, string> = {
    ArrowDown: "ArrowDown",
    ArrowLeft: "ArrowLeft",
    ArrowRight: "ArrowRight",
    ArrowUp: "ArrowUp",
    Backspace: "Backspace",
    Delete: "Delete",
    Enter: "Enter",
    Escape: "Escape",
    Home: "Home",
    End: "End",
    Insert: "Insert",
    PageUp: "PageUp",
    PageDown: "PageDown",
    Space: "Space",
    Tab: "Tab",
  };
  return specialKeys[event.code] ?? "";
}

async function saveShortcut() {
  await stopRecordingShortcut({ restoreAppShortcut: true });
  shortcutMessage.value = null;
  shortcutError.value = null;
  isSavingShortcut.value = true;
  try {
    await store.updateShortcut(shortcutDraft.value);
    shortcutDraft.value = store.shortcut;
    shortcutMessage.value = t("settings.shortcuts.saved");
  } catch (unknownError) {
    shortcutError.value = String(unknownError);
  } finally {
    isSavingShortcut.value = false;
  }
}

function restoreDefaultShortcut() {
  void stopRecordingShortcut({ restoreAppShortcut: true });
  shortcutDraft.value = DEFAULT_SHORTCUT;
  shortcutMessage.value = null;
  shortcutError.value = null;
}

function resetCloudForm() {
  cloudApiAddress.value = store.cloud.apiAddress;
  cloudApiKey.value = store.cloud.apiKey;
  cloudMessage.value = null;
  cloudError.value = null;
}

async function testCloud() {
  cloudMessage.value = null;
  cloudError.value = null;
  isTestingCloud.value = true;
  try {
    await store.testCloudSettings(cloudApiAddress.value, cloudApiKey.value);
    cloudMessage.value = t("settings.cloud.connected");
  } catch (unknownError) {
    cloudError.value = String(unknownError);
  } finally {
    isTestingCloud.value = false;
  }
}

async function saveCloud() {
  cloudMessage.value = null;
  cloudError.value = null;
  isSavingCloud.value = true;
  try {
    await store.saveCloudSettings(cloudApiAddress.value, cloudApiKey.value);
    resetCloudForm();
    cloudMessage.value = t("settings.cloud.saved");
  } catch (unknownError) {
    cloudError.value = String(unknownError);
  } finally {
    isSavingCloud.value = false;
  }
}

async function disableCloud() {
  cloudMessage.value = null;
  cloudError.value = null;
  isSavingCloud.value = true;
  try {
    await store.disableCloudSync();
    resetCloudForm();
    cloudMessage.value = t("settings.cloud.disabledMessage");
  } catch (unknownError) {
    cloudError.value = String(unknownError);
  } finally {
    isSavingCloud.value = false;
  }
}

async function updatePanelOpenBehavior(behavior: PanelOpenBehavior) {
  await store.updatePanelOpenBehavior(behavior);
}

async function updatePanelLayout(layout: PanelLayout) {
  await store.updatePanelLayout(layout);
}

async function updateAppendCopyTimeout(minutes: number) {
  await store.updateAppendCopyTimeout(minutes);
}

async function updateLanguage(language: Language) {
  await store.updateLanguage(language);
}

async function loadOcrStatus() {
  if (isMacOs) return;
  try {
    ocrStatus.value = await ipasteApi.ocrInstallStatus();
    if (ocrStatus.value.installed) {
      lastInstalledOcrMode.value = ocrStatus.value.mode;
    }
  } catch (unknownError) {
    ocrError.value = String(unknownError);
  }
}

async function updateOcrMode(mode: OcrMode) {
  if (mode === store.ocrMode || isInstallingOcr.value || isRemovingOcr.value) return;
  ocrMessage.value = null;
  ocrError.value = null;
  ocrProgress.value = null;
  try {
    await store.updateOcrMode(mode);
    await loadOcrStatus();
  } catch (unknownError) {
    ocrError.value = String(unknownError);
  }
}

async function installOcrAssets() {
  ocrMessage.value = null;
  ocrError.value = null;
  isInstallingOcr.value = true;
  try {
    ocrProgress.value = {
      phase: "fetchingManifest",
      fileName: null,
      downloadedBytes: 0,
      totalBytes: ocrStatus.value?.totalBytes ?? 0,
    };
    ocrStatus.value = await ipasteApi.installOcrAssets();
    lastInstalledOcrMode.value = ocrStatus.value.mode;
    ocrMessage.value = t("ocr.readyMessage");
  } catch (unknownError) {
    ocrError.value = String(unknownError);
  } finally {
    isInstallingOcr.value = false;
  }
}

async function removeOcrAssets() {
  ocrMessage.value = null;
  ocrError.value = null;
  isRemovingOcr.value = true;
  try {
    ocrStatus.value = await ipasteApi.removeOcrAssets();
    ocrProgress.value = null;
    lastInstalledOcrMode.value = null;
    ocrMessage.value = t("ocr.removedMessage");
  } catch (unknownError) {
    ocrError.value = String(unknownError);
  } finally {
    isRemovingOcr.value = false;
  }
}

async function openOcrInstallDir() {
  if (!ocrStatus.value?.installDir) return;
  ocrMessage.value = null;
  ocrError.value = null;
  try {
    await openPath(ocrStatus.value.installDir);
  } catch (unknownError) {
    ocrError.value = String(unknownError);
  }
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 MB";
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

</script>

<template>
  <main class="settings-shell">
    <section class="settings-window">
      <header class="settings-topbar">
        <nav class="settings-tabs" :aria-label="t('settings.tabsLabel')">
          <button
            v-for="tab in tabs"
            :key="tab.id"
            type="button"
            class="settings-tab"
            :class="{ 'settings-tab-active': activeTab === tab.id }"
            @click="activeTab = tab.id"
          >
            <component :is="tab.icon" class="size-4" />
            <span>{{ tab.label }}</span>
          </button>
        </nav>
      </header>

      <div class="settings-content subtle-scrollbar">
        <div v-if="activeTab === 'general'" class="settings-section">
          <section class="settings-panel settings-language-panel items-start">
            <div class="settings-icon settings-icon-teal">
              <Sparkles class="size-5" />
            </div>

            <div class="min-w-0 flex-1">
              <h2 class="text-sm font-semibold text-slate-950">{{ t("settings.language.title") }}</h2>
              <p class="mt-1 text-sm text-slate-500">{{ t("settings.language.description") }}</p>
            </div>

            <LanguageSelect
              class="settings-language-select"
              :model-value="store.language"
              :options="languageOptions"
              :label="t('settings.language.title')"
              @update:model-value="updateLanguage"
            />
          </section>

          <section class="settings-panel items-start">
            <div class="settings-icon settings-icon-blue">
              <SlidersHorizontal class="size-5" />
            </div>

            <div class="min-w-0 flex-1">
              <h2 class="text-sm font-semibold text-slate-950">{{ t("settings.openDefault.title") }}</h2>
              <p class="mt-1 text-sm text-slate-500">{{ t("settings.openDefault.description") }}</p>
            </div>

            <div class="segmented-control">
              <button
                v-for="option in panelOpenOptions"
                :key="option.value"
                type="button"
                class="segmented-option segmented-option-with-icon"
                :class="{ 'segmented-option-active': store.panelOpenBehavior === option.value }"
                @click="updatePanelOpenBehavior(option.value)"
              >
                <component :is="option.icon" class="size-3.5" />
                <span>{{ option.label }}</span>
              </button>
            </div>
          </section>

          <section class="settings-panel settings-column-panel">
            <div class="settings-panel-heading">
              <div class="settings-icon settings-icon-blue">
                <AppWindow class="size-5" />
              </div>
              <div class="min-w-0 flex-1">
                <h2 class="text-sm font-semibold text-slate-950">{{ t("settings.layout.title") }}</h2>
                <p class="mt-1 text-sm text-slate-500">{{ t("settings.layout.description") }}</p>
              </div>
            </div>

            <div class="settings-layout-options">
              <button
                v-for="option in panelLayoutOptions"
                :key="option.value"
                type="button"
                class="layout-option-button"
                :class="{ 'layout-option-button-active': store.panelLayout === option.value }"
                :aria-pressed="store.panelLayout === option.value"
                @click="updatePanelLayout(option.value)"
              >
                <span class="layout-option-preview" :class="`layout-option-preview-${option.value}`">
                  <span class="layout-preview-categories">
                    <span />
                    <span />
                    <span />
                  </span>
                  <span class="layout-preview-list">
                    <span />
                    <span />
                    <span />
                    <span />
                  </span>
                </span>
                <span class="layout-option-label">{{ option.label }}</span>
              </button>
            </div>
          </section>

          <section class="settings-panel settings-column-panel">
            <div class="settings-panel-heading">
              <div class="settings-icon settings-icon-teal">
                <ClipboardPlus class="size-5" />
              </div>
              <div class="min-w-0 flex-1">
                <h2 class="text-sm font-semibold text-slate-950">{{ t("settings.appendCopy.title") }}</h2>
                <p class="mt-1 text-sm text-slate-500">{{ t("settings.appendCopy.description", { duration: appendCopyTimeoutText }) }}</p>
              </div>
            </div>

            <div class="segmented-control settings-retention-control">
              <button
                v-for="option in appendCopyTimeoutOptions"
                :key="option.value"
                type="button"
                class="segmented-option"
                :class="{ 'segmented-option-active': store.appendCopyTimeoutMinutes === option.value }"
                @click="updateAppendCopyTimeout(option.value)"
              >
                {{ option.label }}
              </button>
            </div>
          </section>

          <section class="settings-panel settings-column-panel">
            <div class="settings-panel-heading">
              <div class="settings-icon settings-icon-blue">
                <Database class="size-5" />
              </div>
              <div class="min-w-0">
                <h2 class="text-sm font-semibold text-slate-950">{{ t("settings.storage.title") }}</h2>
                <p class="mt-1 text-sm text-slate-500">{{ t("settings.storage.description", { duration: retentionText }) }}</p>
              </div>
            </div>

            <div class="segmented-control settings-retention-control">
              <button
                v-for="option in retentionOptions"
                :key="option.value"
                type="button"
                class="segmented-option"
                :class="{ 'segmented-option-active': store.retentionDays === option.value }"
                @click="store.updateRetentionDays(option.value)"
              >
                {{ option.label }}
              </button>
            </div>
          </section>

        </div>

        <div v-else-if="activeTab === 'shortcuts'" class="settings-section">
          <section class="settings-panel settings-column-panel">
            <div class="settings-panel-heading">
              <div class="settings-icon settings-icon-teal">
                <Keyboard class="size-5" />
              </div>
              <div class="min-w-0 flex-1">
                <h2 class="text-sm font-semibold text-slate-950">{{ t("settings.shortcuts.global.title") }}</h2>
                <p class="mt-1 text-sm text-slate-500">{{ t("settings.shortcuts.global.description") }}</p>
              </div>
            </div>

            <div class="settings-shortcut-recorder">
              <button
                type="button"
                class="shortcut-capture-button"
                :class="{ 'shortcut-capture-button-recording': shortcutRecording }"
                :aria-pressed="shortcutRecording"
                @click="startRecordingShortcut"
              >
                <Keyboard class="size-4" />
                <span>{{ shortcutRecording ? t("settings.shortcuts.recording") : formattedShortcutDraft }}</span>
              </button>

              <button
                type="button"
                class="settings-action-button"
                :disabled="isSavingShortcut"
                @click="restoreDefaultShortcut"
              >
                <RotateCcw class="size-4" />
                <span>{{ t("settings.shortcuts.restoreDefault") }}</span>
              </button>

              <button
                type="button"
                class="settings-action-button settings-action-button-primary"
                :disabled="!canSaveShortcut"
                @click="saveShortcut"
              >
                <Save class="size-4" />
                <span>{{ isSavingShortcut ? t("common.saving") : t("common.save") }}</span>
              </button>
            </div>

            <p
              v-if="shortcutError || shortcutMessage"
              class="settings-message"
              :class="{ 'settings-message-error': shortcutError }"
            >
              <CheckCircle2 v-if="shortcutMessage && !shortcutError" class="size-4" />
              <AlertCircle v-else class="size-4" />
              <span>{{ shortcutError || shortcutMessage }}</span>
            </p>
          </section>

          <section class="settings-panel settings-column-panel">
            <div class="settings-panel-heading">
              <div class="settings-icon settings-icon-blue">
                <Keyboard class="size-5" />
              </div>
              <div class="min-w-0 flex-1">
                <h2 class="text-sm font-semibold text-slate-950">{{ t("settings.shortcuts.panel.title") }}</h2>
                <p class="mt-1 text-sm text-slate-500">{{ t("settings.shortcuts.panel.description") }}</p>
              </div>
            </div>

            <div class="settings-shortcut-list">
              <div v-for="shortcut in fixedShortcuts" :key="shortcut.action" class="settings-shortcut-row">
                <div class="shortcut-kbd-group" aria-hidden="true">
                  <kbd v-for="key in shortcut.keys" :key="key" class="shortcut-kbd">{{ key }}</kbd>
                </div>
                <span>{{ shortcut.action }}</span>
              </div>
            </div>
          </section>
        </div>

        <div v-else-if="activeTab === 'ocr'" class="settings-section">
          <section v-if="!isMacOs" class="settings-panel settings-column-panel">
            <div class="settings-panel-heading">
              <div class="settings-icon settings-icon-violet">
                <ScanText class="size-5" />
              </div>
              <div class="min-w-0 flex-1">
                <h2 class="text-sm font-semibold text-slate-950">{{ t("settings.tabs.ocr") }}</h2>
                <p class="mt-1 text-sm text-slate-500">{{ ocrStatusText }}</p>
              </div>
              <span class="ocr-status-badge" :class="{ 'ocr-status-badge-ready': ocrStatus?.installed }">
                {{ ocrStatus?.installed ? t("common.ready") : t("common.notInstalled") }}
              </span>
            </div>

            <div class="ocr-mode-options">
              <button
                v-for="option in ocrModeOptions"
                :key="option.value"
                type="button"
                class="ocr-mode-option"
                :class="{ 'ocr-mode-option-active': store.ocrMode === option.value }"
                :aria-pressed="store.ocrMode === option.value"
                :disabled="isInstallingOcr || isRemovingOcr"
                @click="updateOcrMode(option.value)"
              >
                <span class="ocr-mode-option-header">
                  <span>{{ option.label }}</span>
                  <span>{{ formatBytes(option.totalBytes) }}</span>
                </span>
                <span class="ocr-mode-option-description">{{ option.description }}</span>
              </button>
            </div>
            <p class="ocr-mode-hint">
              {{ t("ocr.modeHint") }}
            </p>

            <div class="ocr-install-panel">
              <div class="ocr-install-meter">
                <div class="ocr-install-meter-fill" :style="{ width: `${ocrInstallPercent}%` }" />
              </div>
              <div class="ocr-install-meta">
                <span>{{ ocrDownloadedText }}</span>
                <span>{{ ocrInstallPercent }}%</span>
              </div>
            </div>

            <div class="ocr-install-details">
              <span>{{ t("ocr.downloadContents") }}</span>
              <span>{{ t("ocr.currentSelection", { label: selectedOcrModeOption.label, description: selectedOcrModeOption.description }) }}</span>
              <div v-if="ocrStatus?.installDir" class="ocr-install-dir-row">
                <span>{{ t("ocr.directory", { path: ocrStatus.installDir }) }}</span>
                <button
                  type="button"
                  class="settings-icon-button"
                  :title="t('ocr.openDownloadDir')"
                  :aria-label="t('ocr.openDownloadDir')"
                  @click="openOcrInstallDir"
                >
                  <FolderOpen class="size-4" />
                </button>
              </div>
              <span v-if="ocrStatus?.manifestUrl">{{ t("ocr.manifest", { url: ocrStatus.manifestUrl }) }}</span>
              <span v-if="ocrProgress?.fileName">{{ t("ocr.currentFile", { file: ocrProgress.fileName }) }}</span>
            </div>

            <p v-if="ocrError || ocrMessage" class="settings-message" :class="{ 'settings-message-error': ocrError }">
              <CheckCircle2 v-if="ocrMessage && !ocrError" class="size-4" />
              <AlertCircle v-else class="size-4" />
              <span>{{ ocrError || ocrMessage }}</span>
            </p>

            <div class="settings-action-row">
              <button
                type="button"
                class="settings-action-button settings-action-button-primary"
                :disabled="isInstallingOcr || isRemovingOcr"
                @click="installOcrAssets"
              >
                <LoaderCircle v-if="isInstallingOcr" class="size-4 update-spin" />
                <Download v-else class="size-4" />
                <span>{{ ocrInstallButtonText }}</span>
              </button>
              <button
                type="button"
                class="settings-action-button settings-action-button-danger"
                :disabled="isInstallingOcr || isRemovingOcr || !ocrStatus?.installed"
                @click="removeOcrAssets"
              >
                <Unplug class="size-4" />
                <span>{{ isRemovingOcr ? t("ocr.deleting") : t("ocr.deleteResources") }}</span>
              </button>
            </div>
          </section>
        </div>

        <div v-else-if="activeTab === 'dataManagement'" class="settings-section">
          <div class="data-management-grid">
            <section class="settings-panel settings-column-panel">
              <div class="settings-panel-heading">
                <div class="settings-icon settings-icon-teal">
                  <Cloud class="size-5" />
                </div>
                <div class="min-w-0">
                  <h2 class="text-sm font-semibold text-slate-950">{{ t("settings.cloud.title") }}</h2>
                  <p class="mt-1 text-sm text-slate-500">{{ cloudStatusText }}</p>
                </div>
              </div>

              <p class="sync-hint">
                {{ t("settings.cloud.description") }}
              </p>

              <label class="settings-field">
                <span>{{ t("settings.cloud.apiAddress") }}</span>
                <input v-model="cloudApiAddress" type="url" placeholder="https://your-project.pages.dev" spellcheck="false" />
              </label>

              <label class="settings-field">
                <span>API Key</span>
                <input v-model="cloudApiKey" type="password" autocomplete="current-password" />
              </label>

              <p v-if="cloudError || cloudMessage" class="settings-message" :class="{ 'settings-message-error': cloudError }">
                <CheckCircle2 v-if="cloudMessage && !cloudError" class="size-4" />
                <Unplug v-else class="size-4" />
                <span>{{ cloudError || cloudMessage }}</span>
              </p>

              <div class="settings-action-row">
                <button type="button" class="settings-action-button" :disabled="isTestingCloud" @click="testCloud">
                  <CheckCircle2 class="size-4" />
                  <span>{{ isTestingCloud ? t("settings.cloud.testing") : t("settings.cloud.test") }}</span>
                </button>
                <button type="button" class="settings-action-button settings-action-button-primary" :disabled="isSavingCloud" @click="saveCloud">
                  <Cloud class="size-4" />
                  <span>{{ isSavingCloud ? t("common.saving") : t("settings.cloud.saveAndSync") }}</span>
                </button>
                <button type="button" class="settings-action-button settings-action-button-danger" :disabled="isSavingCloud || !store.cloud.enabled" @click="disableCloud">
                  <Unplug class="size-4" />
                  <span>{{ t("settings.cloud.disable") }}</span>
                </button>
              </div>
            </section>
          </div>
        </div>

        <div v-else-if="activeTab === 'permissions'" class="settings-section">
          <section class="settings-panel items-start">
            <div class="settings-icon settings-icon-blue">
              <Keyboard class="size-5" />
            </div>

            <div class="min-w-0 flex-1">
              <h2 class="text-sm font-semibold text-slate-950">{{ t("settings.permissions.accessibility.title") }}</h2>
              <p class="mt-1 text-sm leading-6 text-slate-500">
                {{ t("settings.permissions.accessibility.description") }}
              </p>
            </div>

            <button
              type="button"
              class="switch-control"
              :class="{ 'switch-control-active': showPermissionGuide }"
              :aria-label="t('settings.permissions.showGuide')"
              @click="openAccessibilityGuide"
            >
              <span />
            </button>
          </section>

          <section v-if="showPermissionGuide" class="permission-guide">
            <h3 class="text-sm font-semibold text-slate-950">{{ t("settings.permissions.howTo") }}</h3>
            <p class="mt-2 text-sm leading-6 text-slate-600">
              {{ t("settings.permissions.guide") }}
            </p>
            <button type="button" class="permission-link" @click="openAccessibilityGuide">
              <span>{{ t("settings.permissions.open") }}</span>
              <ChevronRight class="size-4" />
            </button>
          </section>
        </div>

        <div v-else class="settings-section">
          <section class="settings-panel settings-about-panel">
            <div class="settings-about-header">
              <div class="settings-icon settings-icon-violet">
                <Sparkles class="size-5" />
              </div>
              <div class="min-w-0">
                <h2 class="text-sm font-semibold text-slate-950">iPaste</h2>
                <p class="mt-1 text-sm text-slate-500">{{ t("settings.about.description") }}</p>
              </div>
            </div>

            <section class="about-update-panel" :class="{ 'about-update-panel-error': updater.updateStatus.value === 'error' }">
              <div class="about-update-copy">
                <div class="about-update-icon" :class="{ 'about-update-icon-error': updater.updateStatus.value === 'error' }">
                  <AlertCircle v-if="updater.updateStatus.value === 'error'" class="size-4" />
                  <Download v-else-if="updater.updateStatus.value === 'available' || updater.updateStatus.value === 'downloading'" class="size-4" />
                  <CheckCircle2 v-else-if="updater.updateStatus.value === 'noUpdate' || updater.updateStatus.value === 'ready'" class="size-4" />
                  <RefreshCw v-else class="size-4" />
                </div>
                <div class="min-w-0">
                  <div class="about-update-heading">
                    <h3 class="about-update-title">{{ t("settings.about.softwareUpdate") }}</h3>
                    <span class="about-version-badge">v{{ appInfo?.version ?? "0.1.0" }}</span>
                  </div>
                  <p>{{ updater.updateSummaryText.value }}</p>
                </div>
              </div>

              <button
                type="button"
                class="settings-action-button settings-action-button-primary about-update-button"
                :disabled="updater.isUpdateBusy.value"
                @click="updater.checkForUpdate()"
              >
                <RefreshCw class="size-4" :class="{ 'update-spin': updater.updateStatus.value === 'checking' }" />
                <span>{{ updater.updateButtonText.value }}</span>
              </button>
            </section>

            <div>
              <h3 class="about-label">{{ t("settings.about.techStack") }}</h3>
              <div class="tech-stack-grid">
                <div v-for="item in techStack" :key="item.name" class="tech-stack-item">
                  <div class="tech-stack-icon" :class="`tech-stack-icon-${item.tone}`">
                    <component :is="item.icon" class="size-4" />
                  </div>
                  <div class="min-w-0">
                    <strong>{{ item.name }}</strong>
                    <span>{{ item.detail }}</span>
                  </div>
                </div>
              </div>
            </div>
          </section>
        </div>
      </div>
    </section>

    <UpdateDialog
      :open="updater.updateDialogOpen.value"
      :status="updater.updateStatus.value"
      :update="updater.availableUpdate.value"
      :current-version="appInfo?.version"
      :error="updater.updateError.value"
      :error-phase="updater.updateErrorPhase.value"
      :downloaded-bytes="updater.updateDownloadedBytes.value"
      :total-bytes="updater.updateTotalBytes.value"
      @dismiss="updater.dismissUpdateDialog"
      @install="updater.installAvailableUpdate"
      @relaunch="updater.relaunchForUpdate"
    />
  </main>
</template>
