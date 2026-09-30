<script setup lang="ts">
import {
  File,
  Image,
  Link,
  Maximize2,
  Palette,
  Type,
} from "lucide-vue-next";
import { computed } from "vue";
import { useFileReferencePreview } from "../composables/useFileReferencePreview";
import { useImageThumbnail } from "../composables/useImageThumbnail";
import ContentPinIcon from "./ContentPinIcon.vue";
import { clipFileName, fileIconCategory, fileIconSrc } from "../lib/clipFile";
import { clipImageSrc } from "../lib/clipMedia";
import { t } from "../i18n";
import { categoryDisplayName, clipMetricText, formatFileSize, formatTime, typeLabel } from "../lib/format";
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
const isColor = computed(() => props.item.clipType === "color");
const isFile = computed(() => props.item.clipType === "file");
const isImageFile = computed(() => isFile.value && fileIconCategory(props.item.text) === "image");
const originalImageSrc = computed(() => clipImageSrc(props.item));
const thumbnailSrc = useImageThumbnail(() => isImage.value ? props.item.text : null);
const imageSrc = computed(() => thumbnailSrc.value || originalImageSrc.value);
const colorPreviewValue = computed(() => props.item.text.trim());
// Keep full clipboard content for copying/viewing, but never lay it all out in a card.
const contentText = computed(() => props.item.text);
const fileName = computed(() => clipFileName(contentText.value));
const fileArtworkSrc = computed(() => fileIconSrc(contentText.value));
const displayTitle = computed(() => props.item.displayName?.trim() || "");
const headerLabel = computed(() => displayTitle.value || typeLabel(props.item.clipType));
const clipType = computed(() => props.item.clipType);
const previewText = computed(() => props.item.previewText);
const previewContent = computed(() => (contentText.value || previewText.value).slice(0, 500));
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
const filePreview = useFileReferencePreview(
  () => isFile.value ? contentText.value : null,
  () => props.item.collection === "history" ? props.item.lastCapturedAt : props.item.updatedAt,
);
const cardImageSrc = computed(() => isImage.value ? imageSrc.value : filePreview.thumbnailSrc.value);
const showsImagePreview = computed(() => isImage.value || (isImageFile.value && !!cardImageSrc.value));
const pixelDimensions = computed(() => {
  const dimensions = filePreview.dimensions.value;
  return isImageFile.value && dimensions
    ? `${dimensions.width} × ${dimensions.height}`
    : "";
});
const metricText = computed(() => isImageFile.value ? formatFileSize(filePreview.size.value) : clipMetricText(
  clipType.value,
  contentText.value,
  previewText.value,
  filePreview.size.value,
));

const iconComponent = computed(() => {
  if (props.item.clipType === "link") return Link;
  if (props.item.clipType === "color") return Palette;
  if (props.item.clipType === "image") return Image;
  if (props.item.clipType === "file") return File;
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
    <img
      v-if="isFile && !filePreview.thumbnailSrc.value && !filePreview.error.value"
      class="clip-file-watermark"
      :src="fileArtworkSrc"
      alt=""
      aria-hidden="true"
      draggable="false"
    />
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
          <span
            v-if="item.isPinned"
            class="clip-pin-indicator"
            role="img"
            :aria-label="t('clip.pinned')"
            :data-tooltip="t('clip.pinned')"
          >
            <ContentPinIcon filled class="size-3.5" />
          </span>
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
          v-if="showsImagePreview"
          class="clip-preview-image mt-1.5"
          @pointermove="moveImagePreview"
          @pointerleave="resetImagePreview"
        >
          <img class="w-full object-cover" :src="cardImageSrc" :alt="isImageFile ? fileName : t('common.imagePreviewAlt')" draggable="false" />
        </div>

        <div v-else-if="isColor" class="clip-preview-color mt-1.5">
          <span class="clip-preview-color-swatch" :style="{ backgroundColor: colorPreviewValue }" />
          <span class="clip-preview-color-code">{{ previewContent }}</span>
        </div>

        <div
          v-else-if="isFile"
          class="clip-file-preview mt-1 min-h-0 flex-1 overflow-hidden"
        >
          <div class="clip-file-summary">
            <span
              v-if="!isImageFile"
              class="clip-file-name"
              :title="contentText"
              :data-tooltip="contentText"
            >{{ fileName || typeLabel(item.clipType) }}</span>
            <span v-if="filePreview.error.value" class="clip-file-error" role="status">
              {{ filePreview.error.value }}
            </span>
          </div>
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
    <div class="clip-card-footer" :class="{ 'clip-card-footer-image-file': isImageFile }">
      <span v-if="pixelDimensions" class="clip-metric-badge clip-file-metric">
        <span class="sr-only">{{ metricText }}, {{ pixelDimensions }}</span>
        <span class="clip-file-metric-face clip-file-metric-size" aria-hidden="true">{{ metricText }}</span>
        <span class="clip-file-metric-face clip-file-metric-pixels" aria-hidden="true">{{ pixelDimensions }}</span>
      </span>
      <span v-else class="clip-metric-badge">{{ metricText }}</span>
      <span
        v-if="categoryTagLabel"
        class="clip-category-tag"
        :style="{ '--tag-color': categoryTagColor }"
      >
        <span class="clip-category-tag-dot" />
        {{ categoryTagLabel }}
      </span>
      <span
        v-if="isImageFile"
        class="clip-file-footer-name"
        :title="fileName"
        :data-tooltip="fileName"
      >{{ fileName }}</span>
    </div>
  </article>
</template>
