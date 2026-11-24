<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ClipboardPlus, Download, Grid2X2, Search, Settings, X } from "lucide-vue-next";
import { useTemplateRef } from "vue";
import { t } from "../i18n";
import DownloadProgressBorder from "./DownloadProgressBorder.vue";

const logoUrl = new URL("../../src-tauri/icons/tray-icon@2x.png", import.meta.url).href;

defineProps<{
  searchOpen: boolean;
  settingsOpen: boolean;
  appCenterOpen: boolean;
  appendCopyEnabled: boolean;
  appendCopyTimeoutMinutes: number;
  hasUpdate?: boolean;
  checkingUpdate?: boolean;
  downloadingUpdate?: boolean;
  updateProgress?: number | null;
  updateLabel?: string;
}>();

const emit = defineEmits<{
  toggleSearch: [];
  toggleSettings: [];
  toggleAppCenter: [];
  toggleAppendCopy: [];
  openUpdate: [];
  close: [];
}>();

const isTauri = "__TAURI_INTERNALS__" in window;
const searchTrigger = useTemplateRef<HTMLButtonElement>("searchTrigger");
let dragReleaseTimer: number | null = null;

defineExpose({
  focusSearchTrigger: () => searchTrigger.value?.focus({ preventScroll: true }),
  isSearchTrigger: (target: EventTarget | null) => target instanceof Node && !!searchTrigger.value?.contains(target),
});

async function startWindowDrag(event: MouseEvent) {
  if (!isTauri || event.button !== 0) return;

  event.preventDefault();
  if (dragReleaseTimer !== null) {
    window.clearTimeout(dragReleaseTimer);
    dragReleaseTimer = null;
  }

  void setMainWindowDragging(true);
  try {
    const nativeDragStarted = await startMainWindowDrag();
    if (!nativeDragStarted) {
      await getCurrentWindow().startDragging();
    }
  } finally {
    dragReleaseTimer = window.setTimeout(() => {
      void setMainWindowDragging(false);
      dragReleaseTimer = null;
    }, 900);
  }
}

function setMainWindowDragging(dragging: boolean) {
  return invoke("set_main_window_dragging", { dragging }).catch(() => {});
}

function startMainWindowDrag() {
  return invoke<boolean>("start_main_window_drag").catch(() => false);
}
</script>

<template>
  <header class="top-bar">
    <div class="top-bar-drag-zone" @mousedown="startWindowDrag">
      <img class="top-bar-logo" :src="logoUrl" alt="" />
      <span class="top-bar-brand">iPaste</span>
    </div>

    <button
      ref="searchTrigger"
      type="button"
      class="top-bar-search-trigger"
      aria-haspopup="dialog"
      :aria-expanded="searchOpen"
      :aria-controls="searchOpen ? 'quick-search-panel' : undefined"
      :aria-label="t('shortcut.search')"
      aria-keyshortcuts="Control+F Meta+F"
      @click.stop="emit('toggleSearch')"
    >
      <Search class="size-4 shrink-0" aria-hidden="true" />
      <span class="truncate">{{ t('shortcut.search') }}</span>
    </button>

    <div class="top-bar-spacer" aria-hidden="true" @mousedown="startWindowDrag" />

    <div class="top-bar-actions">
      <button
        v-if="hasUpdate"
        type="button"
        class="icon-button update-icon-button"
        :class="{ 'update-icon-button-checking': checkingUpdate, 'update-icon-button-downloading': downloadingUpdate }"
        tabindex="-1"
        :aria-label="updateLabel ?? t('topBar.openUpdate')"
        :data-tooltip="updateLabel ?? t('topBar.openUpdate')"
        @click.stop="emit('openUpdate')"
      >
        <Download class="size-4" />
        <DownloadProgressBorder
          v-if="downloadingUpdate"
          :progress="updateProgress ?? null"
          :label="updateLabel ?? t('update.button.downloading')"
        />
      </button>

      <button
        type="button"
        class="icon-button app-center-button"
        :class="{ 'app-center-button-active': appCenterOpen }"
        tabindex="-1"
        :aria-pressed="appCenterOpen"
        :aria-label="t('appCenter.toggle')"
        :data-tooltip="t('appCenter.toggle')"
        @click.stop="emit('toggleAppCenter')"
      >
        <Grid2X2 class="size-4" />
      </button>

      <button
        type="button"
        class="icon-button append-copy-button"
        :class="{ 'append-copy-button-active': appendCopyEnabled }"
        tabindex="-1"
        :aria-pressed="appendCopyEnabled"
        :aria-label="appendCopyEnabled ? t('appendCopy.disable') : t('appendCopy.enable')"
        :data-tooltip="appendCopyEnabled ? t('appendCopy.disable') : t('appendCopy.enableTooltip', { minutes: appendCopyTimeoutMinutes })"
        @click.stop="emit('toggleAppendCopy')"
      >
        <ClipboardPlus class="size-4" />
      </button>

      <button
        type="button"
        class="icon-button"
        :class="{ 'bg-slate-100 text-slate-950': settingsOpen }"
        tabindex="-1"
        :aria-label="t('topBar.openSettings')"
        :data-tooltip="t('topBar.openSettings')"
        @click.stop="emit('toggleSettings')"
      >
        <Settings class="size-4" />
      </button>

      <button type="button" class="icon-button" tabindex="-1" :aria-label="t('topBar.closePanel')" :data-tooltip="t('topBar.closePanel')" @click="emit('close')">
        <X class="size-4" />
      </button>
    </div>
  </header>
</template>
