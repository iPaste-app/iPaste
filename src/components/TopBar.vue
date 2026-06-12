<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ClipboardPlus, Download, Search, Settings, X } from "lucide-vue-next";

const logoUrl = new URL("../../src-tauri/icons/32x32.png", import.meta.url).href;

defineProps<{
  modelValue: string;
  shortcut: string;
  settingsOpen: boolean;
  appendCopyEnabled: boolean;
  appendCopyTimeoutMinutes: number;
  hasUpdate?: boolean;
  checkingUpdate?: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
  toggleSettings: [];
  toggleAppendCopy: [];
  openUpdate: [];
  close: [];
}>();

const isTauri = "__TAURI_INTERNALS__" in window;
let dragReleaseTimer: number | null = null;

async function startWindowDrag(event: MouseEvent) {
  if (!isTauri || event.button !== 0) return;

  event.preventDefault();
  if (dragReleaseTimer !== null) {
    window.clearTimeout(dragReleaseTimer);
    dragReleaseTimer = null;
  }

  void setMainWindowDragging(true);
  try {
    await getCurrentWindow().startDragging();
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
</script>

<template>
  <header class="top-bar">
    <div class="top-bar-drag-zone" @mousedown="startWindowDrag">
      <img class="top-bar-logo" :src="logoUrl" alt="" />
      <span class="top-bar-brand">iPaste</span>
    </div>

    <div class="top-bar-spacer" aria-hidden="true" @mousedown="startWindowDrag" />

    <label
      class="search-box"
    >
      <Search class="size-4 shrink-0 text-slate-400" />
      <input
        class="min-w-0 flex-1 bg-transparent text-sm text-slate-950 outline-none placeholder:text-slate-400"
        :value="modelValue"
        tabindex="-1"
        placeholder="搜索剪贴板内容"
        spellcheck="false"
        @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
      />
    </label>

    <div class="hidden items-center gap-1 rounded-lg border border-slate-200 px-2 py-1.5 text-xs text-slate-500 md:flex">
      <Search class="size-3.5" />
      <span>{{ shortcut }}</span>
    </div>

    <button
      v-if="hasUpdate"
      type="button"
      class="icon-button update-icon-button"
      :class="{ 'update-icon-button-checking': checkingUpdate }"
      tabindex="-1"
      aria-label="打开更新"
      data-tooltip="打开更新"
      @click.stop="emit('openUpdate')"
    >
      <Download class="size-4" />
    </button>

    <button
      type="button"
      class="icon-button append-copy-button"
      :class="{ 'append-copy-button-active': appendCopyEnabled }"
      tabindex="-1"
      :aria-pressed="appendCopyEnabled"
      :aria-label="appendCopyEnabled ? '关闭追加复制' : '开启追加复制'"
      :data-tooltip="appendCopyEnabled ? '关闭追加复制' : `开启追加复制；${appendCopyTimeoutMinutes} 分钟后自动关闭`"
      @click.stop="emit('toggleAppendCopy')"
    >
      <ClipboardPlus class="size-4" />
    </button>

    <button
      type="button"
      class="icon-button"
      :class="{ 'bg-slate-100 text-slate-950': settingsOpen }"
      tabindex="-1"
      aria-label="打开设置"
      data-tooltip="打开设置"
      @click.stop="emit('toggleSettings')"
    >
      <Settings class="size-4" />
    </button>

    <button type="button" class="icon-button" tabindex="-1" aria-label="关闭面板" data-tooltip="关闭面板" @click="emit('close')">
      <X class="size-4" />
    </button>
  </header>
</template>
