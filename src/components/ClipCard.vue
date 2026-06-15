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

const isHistory = computed(() => props.item.collection === "history");
const isColor = computed(() => props.item.clipType === "color");
const isImage = computed(() => props.item.clipType === "image");
const imageSrc = computed(() => clipImageSrc(props.item));
const displayTitle = computed(() => props.item.displayName?.trim() || "");
const headerLabel = computed(() => displayTitle.value || typeLabel(props.item.clipType));
const previewContent = computed(() => props.item.text || props.item.previewText);
const shouldFadePreview = computed(() => previewContent.value.length > 28 || previewContent.value.includes("\n"));
const historyCategoryLabel = computed(() => {
  if (!isHistory.value || !props.categoryTags.length) return "";

  const [firstCategory] = props.categoryTags;
  const label = categoryDisplayName(firstCategory.name);
  const extraCount = props.categoryTags.length - 1;
  return extraCount > 0 ? `${label} +${extraCount}` : label;
});
const historyCategoryColor = computed(() => {
  if (!isHistory.value || !props.categoryTags.length) return undefined;
  return props.categoryTags[0].color;
});
const displayTime = computed(() =>
  props.item.collection === "history" ? props.item.lastCapturedAt : props.item.createdAt,
);

const iconComponent = computed(() => {
  if (props.item.clipType === "link") return Link;
  if (props.item.clipType === "color") return Palette;
  if (props.item.clipType === "image") return Image;
  if (props.item.clipType === "file") return FileText;
  return Type;
});

const colorValue = computed(() => {
  if (!isColor.value) return "#CBD5E1";
  return props.item.text.trim();
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
    :class="{ 'clip-card-selected': selected }"
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
    <div class="clip-card-main flex items-start gap-2.5">
      <button
        type="button"
        class="clip-type-handle"
        :class="{
          'border-teal-200 bg-teal-50 text-teal-700': selected,
          'clip-type-handle-reorderable': reorderEnabled,
        }"
        :aria-label="reorderEnabled ? t('clip.dragReorder') : typeLabel(item.clipType)"
        :data-tooltip="reorderEnabled ? t('clip.dragReorder') : typeLabel(item.clipType)"
        tabindex="-1"
        @click.stop="emit('select', index)"
        @dblclick.stop="emit('apply', item)"
        @pointerdown.stop="startReorder"
      >
        <span
          v-if="isColor"
          class="size-5 rounded-full border border-white shadow-sm"
          :style="{ backgroundColor: colorValue }"
        />
        <img
          v-else-if="isImage"
          class="size-6 rounded-md object-cover"
          :src="imageSrc"
          alt=""
        />
        <component :is="iconComponent" v-else class="size-4" />
      </button>

      <div class="clip-card-content min-w-0 flex-1">
        <div class="flex items-center gap-2 pr-8">
          <span class="clip-card-title min-w-0 truncate text-xs">{{ headerLabel }}</span>
          <span class="size-1 rounded-full bg-slate-300" />
          <span class="truncate text-xs text-slate-400">{{ formatTime(displayTime) }}</span>
          <span
            v-if="historyCategoryLabel"
            class="clip-category-tag ml-auto"
            :style="{ '--tag-color': historyCategoryColor }"
          >
            <span class="clip-category-tag-dot" />
            {{ historyCategoryLabel }}
          </span>
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
    <span class="clip-metric-badge">{{ isImage ? item.previewText : textStats(item.text) }}</span>
  </article>
</template>
