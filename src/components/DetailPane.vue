<script setup lang="ts">
import { Clipboard, Info, Link } from "lucide-vue-next";
import { computed } from "vue";
import { useFileReferencePreview } from "../composables/useFileReferencePreview";
import { clipFileName, fileIconSrc } from "../lib/clipFile";
import { clipImageSrc } from "../lib/clipMedia";
import { t } from "../i18n";
import { clipMetricText, formatTime, typeLabel } from "../lib/format";
import type { ClipViewItem } from "../types";

const props = defineProps<{
  item?: ClipViewItem;
}>();

const lines = computed(() => props.item?.text.split(/\r?\n/).length ?? 0);
const isImage = computed(() => props.item?.clipType === "image");
const isFile = computed(() => props.item?.clipType === "file");
const imageSrc = computed(() => props.item ? clipImageSrc(props.item) : "");
const fileArtworkSrc = computed(() => fileIconSrc(props.item?.text ?? ""));
const filePreview = useFileReferencePreview(
  () => isFile.value ? props.item?.text ?? null : null,
  () => {
    const current = props.item;
    if (!current) return "";
    return current.collection === "history" ? current.lastCapturedAt : current.updatedAt;
  },
);
const detailTitle = computed(() => {
  if (!props.item) return "";
  if (props.item.clipType === "file") return clipFileName(props.item.text) || typeLabel(props.item.clipType);
  return props.item.displayName?.trim() || t("clip.clipboardTitle", { type: typeLabel(props.item.clipType) });
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
          {{ t("detail.title") }}
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
          <img class="max-h-[360px] w-full object-contain" :src="imageSrc" :alt="t('common.imagePreviewAlt')" />
        </div>

        <section v-else-if="isFile" class="mb-4 rounded-lg border border-slate-200 bg-slate-50 p-3">
          <img
            v-if="filePreview.thumbnailSrc.value"
            :src="filePreview.thumbnailSrc.value"
            class="mb-3 max-h-[240px] w-full rounded-md object-contain"
            :alt="detailTitle"
            draggable="false"
          />
          <div class="flex items-center gap-2 text-xs font-medium text-slate-500">
            <img :src="fileArtworkSrc" class="h-9 w-auto shrink-0 object-contain" alt="" aria-hidden="true" draggable="false" />
            <span>{{ t("file.path") }}</span>
          </div>
          <p class="mt-1 select-text break-all text-sm leading-5 text-slate-700" :title="item.text">{{ item.text }}</p>
          <p v-if="filePreview.error.value" class="mt-2 text-xs leading-4 text-red-700" role="status">
            {{ filePreview.error.value }}
          </p>
          <p v-else-if="filePreview.size.value !== null" class="mt-2 text-xs text-slate-500">
            {{ clipMetricText(item.clipType, item.text, item.previewText, filePreview.size.value) }}
          </p>
        </section>

        <pre
          v-if="!isImage && !isFile"
          class="max-h-[320px] overflow-auto whitespace-pre-wrap break-words rounded-lg border border-slate-200 bg-slate-50 p-3 text-sm leading-5 text-slate-800"
        >{{ item.text }}</pre>

        <dl v-if="!isFile" class="mt-4 grid grid-cols-2 gap-3 text-sm text-slate-400">
          <div>
            <dt class="text-xs text-slate-400">{{ t("common.size") }}</dt>
            <dd class="mt-1 text-slate-500">{{ clipMetricText(item.clipType, item.text, item.previewText) }}</dd>
          </div>
          <div v-if="!isImage">
            <dt class="text-xs text-slate-400">{{ t("common.lines") }}</dt>
            <dd class="mt-1 text-slate-500">{{ lines }}</dd>
          </div>
        </dl>
      </div>
    </div>

    <div v-else class="flex h-full flex-col items-center justify-center gap-3 px-8 text-center text-slate-400">
      <Clipboard class="size-8" />
      <p class="text-sm">{{ t("detail.noSelection") }}</p>
    </div>
  </aside>
</template>
