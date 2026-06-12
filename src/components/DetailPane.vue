<script setup lang="ts">
import { Clipboard, Info, Link } from "lucide-vue-next";
import { computed } from "vue";
import { clipImageSrc } from "../lib/clipMedia";
import { formatTime, textStats, typeLabel } from "../lib/format";
import type { ClipViewItem } from "../types";

const props = defineProps<{
  item?: ClipViewItem;
}>();

const lines = computed(() => props.item?.text.split(/\r?\n/).length ?? 0);
const isImage = computed(() => props.item?.clipType === "image");
const imageSrc = computed(() => props.item ? clipImageSrc(props.item) : "");
const detailTitle = computed(() => {
  if (!props.item) return "";
  return props.item.displayName?.trim() || `${typeLabel(props.item.clipType)}剪贴板`;
});
const displayTime = computed(() => {
  if (!props.item) return "";
  return props.item.collection === "history" ? props.item.lastCapturedAt : props.item.createdAt;
});
</script>

<template>
  <aside class="hidden w-60 shrink-0 border-l border-slate-200 bg-white/80 lg:block">
    <div v-if="item" class="flex h-full flex-col">
      <div class="border-b border-slate-200 p-4">
        <div class="flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.08em] text-slate-400">
          <Info class="size-3.5" />
          详情
        </div>
        <h2 class="mt-2 truncate text-base font-semibold text-slate-950">
          {{ detailTitle }}
        </h2>
        <p class="mt-1 text-xs text-slate-500">{{ formatTime(displayTime) }}</p>
      </div>

      <div class="flex-1 overflow-y-auto p-4">
        <div
          v-if="item.clipType === 'color'"
          class="mb-4 h-24 rounded-lg border border-slate-200"
          :style="{ backgroundColor: item.text.trim() }"
        />

        <a
          v-if="item.clipType === 'link'"
          class="mb-4 flex items-center gap-2 rounded-lg border border-slate-200 px-3 py-2 text-sm text-teal-700 transition hover:bg-teal-50"
          :href="item.text"
          target="_blank"
          tabindex="-1"
        >
          <Link class="size-4" />
          <span class="truncate">{{ item.text }}</span>
        </a>

        <div
          v-if="isImage"
          class="mb-4 overflow-hidden rounded-xl border border-slate-200 bg-slate-50"
        >
          <img class="max-h-[360px] w-full object-contain" :src="imageSrc" alt="图片剪贴板预览" />
        </div>

        <pre
          v-if="!isImage"
          class="max-h-[320px] overflow-auto whitespace-pre-wrap break-words rounded-lg border border-slate-200 bg-slate-50 p-3 text-sm leading-5 text-slate-800"
        >{{ item.text }}</pre>

        <dl class="mt-4 grid grid-cols-2 gap-3 text-sm text-slate-400">
          <div>
            <dt class="text-xs text-slate-400">大小</dt>
            <dd class="mt-1 text-slate-500">{{ isImage ? item.previewText : textStats(item.text) }}</dd>
          </div>
          <div v-if="!isImage">
            <dt class="text-xs text-slate-400">行数</dt>
            <dd class="mt-1 text-slate-500">{{ lines }}</dd>
          </div>
        </dl>
      </div>
    </div>

    <div v-else class="flex h-full flex-col items-center justify-center gap-3 px-8 text-center text-slate-400">
      <Clipboard class="size-8" />
      <p class="text-sm">未选择剪贴板内容</p>
    </div>
  </aside>
</template>
