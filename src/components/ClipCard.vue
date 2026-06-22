<script setup lang="ts">
import {
  FileText,
  Image,
  Link,
  Maximize2,
  Palette,
  Type,
} from "lucide-vue-next";
import { computed } from "vue";
import { clipImageSrc } from "../lib/clipMedia";
import { t } from "../i18n";
import { categoryDisplayName, formatTime, textStats, typeLabel } from "../lib/format";
import type { Category, ClipViewItem } from "../types";

const props = defineProps<{
  item: ClipViewItem;
  index: number;
  selected: boolean;
  categoryTags: Category[];
  editingName: string | null;
  reorderEnabled: boolean;
}>();

const emit = defineEmits<{
  select: [index: number];
  apply: [item: ClipViewItem];
  expand: [item: ClipViewItem];
  openContextMenu: [payload: { item: ClipViewItem; index: number; x: number; y: number }];
  updateEditingName: [value: string];
  commitRename: [item: ClipViewItem];
  cancelRename: [];
  reorderPointerDown: [payload: { item: ClipViewItem; index: number; event: PointerEvent }];
}>();

const isImage = computed(() => props.item.clipType === "image");
const imageSrc = computed(() => clipImageSrc(props.item));
const displayTitle = computed(() => props.item.displayName?.trim() || "");
const headerLabel = computed(() => displayTitle.value || typeLabel(props.item.clipType));
const previewContent = computed(() => props.item.text || props.item.previewText);
const shouldFadePreview = computed(() => previewContent.value.length > 28 || previewContent.value.includes("\n"));
const categoryTagLabel = computed(() => {
  if (props.item.collection !== "history") return "";
  if (!props.categoryTags.length) return "";
  const [firstCategory] = props.categoryTags;
  const label = categoryDisplayName(firstCategory.name);
  const extraCount = props.categoryTags.length - 1;
  return extraCount > 0 ? `${label} +${extraCount}` : label;
});
const categoryTagColor = computed(() => {
  if (props.item.collection !== "history") return undefined;
  if (!props.categoryTags.length) return undefined;
  return props.categoryTags[0].color;
});
const displayTime = computed(() =>
  props.item.collection === "history" ? props.item.lastCapturedAt : props.item.createdAt,
);
const metricText = computed(() => {
  if (!isImage.value) return textStats(props.item.text);

  const trimmed = props.item.previewText.trim();
  return trimmed.replace(/^(?:image|图片)\s*[:：-]?\s*/i, "").trim() || trimmed;
});

const iconComponent = computed(() => {
  if (props.item.clipType === "link") return Link;
  if (props.item.clipType === "color") return Palette;
  if (props.item.clipType === "image") return Image;
  if (props.item.clipType === "file") return FileText;
  return Type;
});

function openContextMenu(event: MouseEvent) {
  emit("openContextMenu", {
    item: props.item,
    index: props.index,
    x: event.clientX,
    y: event.clientY,
  });
}

function startReorder(event: PointerEvent) {
  if (!props.reorderEnabled) {
    event.preventDefault();
    return;
  }

  emit("reorderPointerDown", {
    item: props.item,
    index: props.index,
    event,
  });
}

function moveImagePreview(event: PointerEvent) {
  const target = event.currentTarget as HTMLElement;
  const rect = target.getBoundingClientRect();
  const x = Math.min(Math.max((event.clientX - rect.left) / rect.width, 0), 1) * 100;
  const y = Math.min(Math.max((event.clientY - rect.top) / rect.height, 0), 1) * 100;

  target.style.setProperty("--clip-image-x", `${x.toFixed(1)}%`);
  target.style.setProperty("--clip-image-y", `${y.toFixed(1)}%`);
  target.style.setProperty("--clip-image-scale", "1.5");
}

function resetImagePreview(event: PointerEvent) {
  const target = event.currentTarget as HTMLElement;
  target.style.removeProperty("--clip-image-x");
  target.style.removeProperty("--clip-image-y");
  target.style.removeProperty("--clip-image-scale");
}
</script>

<template>
  <article
    class="clip-card group"
    :class="[{ 'clip-card-selected': selected }, `clip-card-type-${item.clipType}`]"
    role="option"
    :aria-selected="selected"
    @click="emit('select', index)"
    @dblclick="emit('apply', item)"
    @contextmenu.prevent.stop="openContextMenu"
  >
    <button
      type="button"
      class="clip-expand-button"
      :aria-label="t('clip.expand')"
      :data-tooltip="t('clip.expand')"
      tabindex="-1"
      @click.stop="emit('expand', item)"
    >
      <Maximize2 class="size-3.5" />
    </button>
    <div class="clip-card-main">
      <div class="clip-card-content min-w-0">
        <div class="flex items-center gap-2 pr-8">
          <button
            type="button"
            class="clip-title-type-icon"
            :class="{ 'clip-title-type-icon-reorderable': reorderEnabled }"
            :aria-label="reorderEnabled ? t('clip.dragReorder') : typeLabel(item.clipType)"
            :data-tooltip="reorderEnabled ? t('clip.dragReorder') : typeLabel(item.clipType)"
            tabindex="-1"
            @click.stop="emit('select', index)"
            @dblclick.stop="emit('apply', item)"
            @pointerdown.stop="startReorder"
          >
            <svg
              v-if="item.clipType === 'text'"
              xmlns="http://www.w3.org/2000/svg"
              viewBox="0 0 1024 1024"
              fill="currentColor"
              class="clip-title-type-icon-svg clip-title-type-icon-svg-text"
              aria-hidden="true"
            >
              <path d="M853.333333 170.666667H170.666667a42.666667 42.666667 0 0 0-42.666667 42.666666v128a42.666667 42.666667 0 0 0 85.333333 0V256h256v554.666667H384a42.666667 42.666667 0 0 0 0 85.333333h256a42.666667 42.666667 0 0 0 0-85.333333h-85.333333V256h256v85.333333a42.666667 42.666667 0 0 0 85.333333 0V213.333333a42.666667 42.666667 0 0 0-42.666667-42.666666z" />
            </svg>
            <component :is="iconComponent" v-else class="clip-title-type-icon-svg" />
          </button>
          <span class="clip-card-title min-w-0 truncate text-xs">{{ headerLabel }}</span>
          <span class="size-1 rounded-full bg-slate-300" />
          <span class="truncate text-xs text-slate-400">{{ formatTime(displayTime) }}</span>
        </div>

        <input
          v-if="editingName !== null"
          class="clip-title-input mt-1"
          :value="editingName"
          tabindex="-1"
          spellcheck="false"
          @click.stop
          @dblclick.stop
          @input="emit('updateEditingName', ($event.target as HTMLInputElement).value)"
          @keydown.enter.prevent.stop="emit('commitRename', item)"
          @keydown.escape.prevent.stop="emit('cancelRename')"
          @blur="emit('commitRename', item)"
        />

        <div
          v-if="isImage"
          class="clip-preview-image mt-1.5"
          @pointermove="moveImagePreview"
          @pointerleave="resetImagePreview"
        >
          <img class="w-full object-cover" :src="imageSrc" :alt="t('common.imagePreviewAlt')" />
        </div>

        <p
          v-else
          class="clip-preview-text mt-1 whitespace-pre-wrap break-words text-sm leading-5"
          :class="{ 'clip-preview-text-fade': shouldFadePreview }"
        >
          {{ previewContent }}
        </p>
      </div>
    </div>
    <div class="clip-card-footer">
      <span class="clip-metric-badge">{{ metricText }}</span>
      <span
        v-if="categoryTagLabel"
        class="clip-category-tag"
        :style="{ '--tag-color': categoryTagColor }"
      >
        <span class="clip-category-tag-dot" />
        {{ categoryTagLabel }}
      </span>
    </div>
  </article>
</template>
