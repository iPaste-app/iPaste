<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { Check, Palette, Pencil, Plus, Trash2 } from "lucide-vue-next";
import { t } from "../i18n";
import { categoryDisplayName } from "../lib/format";
import type { Category } from "../types";

const CATEGORY_COLOR_OPTIONS = [
  "#2563EB",
  "#0891B2",
  "#0D9488",
  "#059669",
  "#65A30D",
  "#CA8A04",
  "#D97706",
  "#EA580C",
  "#DC2626",
  "#E11D48",
  "#DB2777",
  "#C026D3",
  "#9333EA",
  "#7C3AED",
  "#4F46E5",
  "#0284C7",
  "#475569",
  "#334155",
  "#111827",
  "#78716C",
  "#A16207",
  "#BE123C",
  "#6D28D9",
  "#0F766E",
];

const props = defineProps<{
  categories: Category[];
  selectedCategoryId: string;
  editingCategoryId: string | null;
  historyCount: number;
  categoryCounts: Record<string, number>;
  orientation?: "horizontal" | "vertical";
}>();

const emit = defineEmits<{
  select: [id: string];
  create: [];
  edit: [id: string];
  rename: [category: Category, name: string];
  recolor: [category: Category, color: string];
  finishEditing: [];
  delete: [id: string];
  reorder: [categoryIds: string[]];
}>();

const editingName = ref("");
const railElement = ref<HTMLElement | null>(null);
const categoryScroller = ref<HTMLElement | null>(null);
const pendingDeleteCategoryId = ref<string | null>(null);
const editingColorCategoryId = ref<string | null>(null);
const colorPopoverPosition = ref({ left: 0, top: 0 });
const isCategoryScrolling = ref(false);
const committedEditingId = ref<string | null>(null);
const draggingCategoryId = ref<string | null>(null);
const categoryDropTargetId = ref<string | null>(null);
const categoryDropSide = ref<"before" | "after" | null>(null);
const categoryDragOffset = ref({ x: 0, y: 0 });
const categoryMenu = ref<{ category: Category; left: number; top: number } | null>(null);
const categoryMenuAnchorRect = ref<DOMRect | null>(null);
let categoryDragState: {
  id: string;
  startX: number;
  startY: number;
  width: number;
  height: number;
  hasMoved: boolean;
  targetId: string | null;
  side: "before" | "after" | null;
} | null = null;
let focusTimer: number | null = null;
let categoryScrollTimer: number | null = null;
let suppressNextCategoryClick = false;

watch(
  () => props.editingCategoryId,
  async (id) => {
    if (!id) return;

    const category = props.categories.find((item) => item.id === id);
    editingName.value = category ? categoryDisplayName(category.name) : "";
    committedEditingId.value = null;
    await nextTick();
    scrollCategoryIntoView(id);
    focusEditingInput();
  },
);

watch(
  () => props.categories.map((category) => category.id),
  (ids) => {
    if (pendingDeleteCategoryId.value && !ids.includes(pendingDeleteCategoryId.value)) {
      pendingDeleteCategoryId.value = null;
    }
    if (editingColorCategoryId.value && !ids.includes(editingColorCategoryId.value)) {
      editingColorCategoryId.value = null;
    }
  },
);

watch(
  () => props.selectedCategoryId,
  async (id) => {
    if (id === "history") return;
    await nextTick();
    scrollCategoryIntoView(id);
  },
);

onMounted(() => {
  window.addEventListener("resize", closeFloatingLayers);
  document.addEventListener("click", closeColorPicker);
  document.addEventListener("click", closeCategoryMenu);
});

onUnmounted(() => {
  window.removeEventListener("resize", closeFloatingLayers);
  document.removeEventListener("click", closeColorPicker);
  document.removeEventListener("click", closeCategoryMenu);
  clearCategoryScrollTimer();
  cleanupCategoryDrag();
  if (focusTimer !== null) {
    window.clearTimeout(focusTimer);
    focusTimer = null;
  }
});

function focusEditingInput() {
  if (focusTimer !== null) {
    window.clearTimeout(focusTimer);
  }

  focusTimer = window.setTimeout(() => {
    const input = railElement.value?.querySelector<HTMLInputElement>(".category-chip-input");
    input?.focus();
    focusTimer = null;
  }, 40);
}

function scrollCategoryIntoView(id: string) {
  const scroller = categoryScroller.value;
  const chip = railElement.value?.querySelector<HTMLElement>(`[data-category-id="${id}"]`);
  if (!scroller || !chip) return;

  const scrollerRect = scroller.getBoundingClientRect();
  const chipRect = chip.getBoundingClientRect();
  const edgePadding = 12;

  if (props.orientation === "vertical") {
    if (chipRect.top < scrollerRect.top + edgePadding) {
      scroller.scrollBy({ top: chipRect.top - scrollerRect.top - edgePadding, behavior: "smooth" });
      showCategoryScrollbar();
      return;
    }

    if (chipRect.bottom > scrollerRect.bottom - edgePadding) {
      scroller.scrollBy({ top: chipRect.bottom - scrollerRect.bottom + edgePadding, behavior: "smooth" });
      showCategoryScrollbar();
    }
    return;
  }

  if (chipRect.left < scrollerRect.left + edgePadding) {
    scroller.scrollBy({ left: chipRect.left - scrollerRect.left - edgePadding, behavior: "smooth" });
    showCategoryScrollbar();
    return;
  }

  if (chipRect.right > scrollerRect.right - edgePadding) {
    scroller.scrollBy({ left: chipRect.right - scrollerRect.right + edgePadding, behavior: "smooth" });
    showCategoryScrollbar();
  }
}

function commitEditing(category: Category) {
  if (committedEditingId.value === category.id) return;
  committedEditingId.value = category.id;

  const name = editingName.value.trim();
  if (name && name !== categoryDisplayName(category.name)) {
    emit("rename", category, name);
  }
  emit("finishEditing");
}

function editCategory(category: Category) {
  pendingDeleteCategoryId.value = null;
  editingColorCategoryId.value = null;
  categoryMenu.value = null;
  committedEditingId.value = null;
  emit("edit", category.id);
}

function selectCategory(id: string) {
  if (suppressNextCategoryClick) {
    suppressNextCategoryClick = false;
    return;
  }

  pendingDeleteCategoryId.value = null;
  editingColorCategoryId.value = null;
  categoryMenuAnchorRect.value = null;
  categoryMenu.value = null;
  emit("select", id);
}

function requestDeleteCategory(id: string, options: { keepMenuOpen?: boolean } = {}) {
  editingColorCategoryId.value = null;

  if (pendingDeleteCategoryId.value === id) {
    pendingDeleteCategoryId.value = null;
    categoryMenu.value = null;
    emit("delete", id);
    return;
  }

  pendingDeleteCategoryId.value = id;
  if (!options.keepMenuOpen) {
    categoryMenu.value = null;
  }
}

function openColorPicker(category: Category, event: MouseEvent) {
  pendingDeleteCategoryId.value = null;
  categoryMenu.value = null;

  const trigger = event.currentTarget as HTMLElement;
  const chip = railElement.value?.querySelector<HTMLElement>(`[data-category-id="${category.id}"]`);
  const rect = chip?.getBoundingClientRect() ?? categoryMenuAnchorRect.value ?? trigger.getBoundingClientRect();
  const popoverWidth = 184;
  const estimatedPopoverHeight = 196;
  const padding = 8;
  const left = props.orientation === "vertical"
    ? rect.right + 8
    : rect.left;
  const top = props.orientation === "vertical"
    ? rect.top - 8
    : rect.bottom + 8;
  colorPopoverPosition.value = {
    left: Math.min(Math.max(left, padding), window.innerWidth - popoverWidth - padding),
    top: Math.min(Math.max(top, padding), window.innerHeight - estimatedPopoverHeight - padding),
  };
  editingColorCategoryId.value =
    editingColorCategoryId.value === category.id ? null : category.id;
}

function closeColorPicker() {
  editingColorCategoryId.value = null;
  categoryMenuAnchorRect.value = null;
}

function updateColor(category: Category, color: string) {
  if (color.toLowerCase() !== category.color.toLowerCase()) {
    emit("recolor", category, color);
  }
  closeColorPicker();
}

function handleCategoryWheel(event: WheelEvent) {
  const scroller = categoryScroller.value;
  if (!scroller) return;

  if (props.orientation === "vertical") {
    showCategoryScrollbar();
    return;
  }

  const canScroll = scroller.scrollWidth > scroller.clientWidth;
  if (!canScroll) return;

  const delta = Math.abs(event.deltaY) >= Math.abs(event.deltaX) ? event.deltaY : event.deltaX;
  if (!delta) return;

  event.preventDefault();
  showCategoryScrollbar();
  scroller.scrollLeft += delta;
}

function activateCategoryScrollbar() {
  isCategoryScrolling.value = true;
}

function scheduleHideCategoryScrollbar() {
  clearCategoryScrollTimer();
  categoryScrollTimer = window.setTimeout(() => {
    isCategoryScrolling.value = false;
    categoryScrollTimer = null;
  }, 520);
}

function showCategoryScrollbar() {
  clearCategoryScrollTimer();
  isCategoryScrolling.value = true;
  scheduleHideCategoryScrollbar();
}

function clearCategoryScrollTimer() {
  if (categoryScrollTimer === null) return;
  window.clearTimeout(categoryScrollTimer);
  categoryScrollTimer = null;
}

function startCategoryDrag(category: Category, event: PointerEvent) {
  if (props.editingCategoryId || event.button !== 0) return;

  const chip = (event.currentTarget as HTMLElement).closest<HTMLElement>("[data-category-id]");
  const rect = chip?.getBoundingClientRect();
  pendingDeleteCategoryId.value = null;
  editingColorCategoryId.value = null;
  categoryMenu.value = null;
  categoryDragState = {
    id: category.id,
    startX: event.clientX,
    startY: event.clientY,
    width: rect?.width ?? 0,
    height: rect?.height ?? 0,
    hasMoved: false,
    targetId: null,
    side: null,
  };
  categoryDragOffset.value = { x: 0, y: 0 };
  window.addEventListener("pointermove", handleCategoryPointerMove);
  window.addEventListener("pointerup", finishCategoryDrag);
  window.addEventListener("pointercancel", cancelCategoryDrag);
}

function handleCategoryPointerMove(event: PointerEvent) {
  const state = categoryDragState;
  if (!state) return;

  if (Math.hypot(event.clientX - state.startX, event.clientY - state.startY) > 3) {
    if (!state.hasMoved) {
      draggingCategoryId.value = state.id;
    }
    state.hasMoved = true;
  }
  if (!state.hasMoved) return;

  event.preventDefault();
  categoryDragOffset.value = {
    x: event.clientX - state.startX,
    y: event.clientY - state.startY,
  };

  const target = categoryTargetFromPosition(state.id, event.clientX, event.clientY);
  if (!target) {
    state.targetId = null;
    state.side = null;
    categoryDropTargetId.value = null;
    categoryDropSide.value = null;
    return;
  }

  state.targetId = target.id;
  state.side = target.side;
  categoryDropTargetId.value = target.id;
  categoryDropSide.value = target.side;
  scrollCategoriesNearPointer(event.clientX, event.clientY);
  showCategoryScrollbar();
}

function finishCategoryDrag(event?: PointerEvent) {
  event?.preventDefault();

  const state = categoryDragState;
  if (state?.hasMoved) {
    suppressNextCategoryClick = true;
    window.setTimeout(() => {
      suppressNextCategoryClick = false;
    }, 0);
  }
  cleanupCategoryDrag();
  if (!state?.hasMoved || !state.targetId || !state.side || state.id === state.targetId) return;

  const ids = props.categories.map((item) => item.id);
  const nextIds = ids.filter((id) => id !== state.id);
  const targetIndex = nextIds.indexOf(state.targetId);
  if (targetIndex < 0) return;

  nextIds.splice(state.side === "after" ? targetIndex + 1 : targetIndex, 0, state.id);
  if (nextIds.join("\n") !== ids.join("\n")) {
    emit("reorder", nextIds);
  }
}

function cancelCategoryDrag() {
  cleanupCategoryDrag();
}

function cleanupCategoryDrag() {
  window.removeEventListener("pointermove", handleCategoryPointerMove);
  window.removeEventListener("pointerup", finishCategoryDrag);
  window.removeEventListener("pointercancel", cancelCategoryDrag);
  categoryDragState = null;
  draggingCategoryId.value = null;
  categoryDropTargetId.value = null;
  categoryDropSide.value = null;
  categoryDragOffset.value = { x: 0, y: 0 };
}

function categoryTargetFromPosition(draggedId: string, clientX: number, clientY: number) {
  const scroller = categoryScroller.value;
  const rail = railElement.value;
  if (!scroller || !rail) return null;

  const scrollerRect = scroller.getBoundingClientRect();
  const tolerance = 28;
  if (props.orientation === "vertical") {
    if (clientX < scrollerRect.left - tolerance || clientX > scrollerRect.right + tolerance) {
      return null;
    }
  } else if (clientY < scrollerRect.top - tolerance || clientY > scrollerRect.bottom + tolerance) {
    return null;
  }

  const targets = Array.from(rail.querySelectorAll<HTMLElement>("[data-category-id]"))
    .map((chip) => {
      const id = chip.dataset.categoryId;
      return id && id !== draggedId ? { id, rect: chip.getBoundingClientRect() } : null;
    })
    .filter((item): item is { id: string; rect: DOMRect } => Boolean(item));

  if (!targets.length) return null;

  const beforeTarget = targets.find((target) =>
    props.orientation === "vertical"
      ? clientY < target.rect.top + target.rect.height / 2
      : clientX < target.rect.left + target.rect.width / 2,
  );
  if (beforeTarget) {
    return {
      id: beforeTarget.id,
      rect: beforeTarget.rect,
      side: "before" as const,
    };
  }

  const lastTarget = targets[targets.length - 1];
  return {
    id: lastTarget.id,
    rect: lastTarget.rect,
    side: "after" as const,
  };
}

function scrollCategoriesNearPointer(clientX: number, clientY: number) {
  const scroller = categoryScroller.value;
  if (!scroller) return;

  const rect = scroller.getBoundingClientRect();
  const edge = 28;
  if (props.orientation === "vertical") {
    if (clientY < rect.top + edge) {
      scroller.scrollTop -= 12;
    } else if (clientY > rect.bottom - edge) {
      scroller.scrollTop += 12;
    }
    return;
  }

  if (clientX < rect.left + edge) {
    scroller.scrollLeft -= 12;
  } else if (clientX > rect.right - edge) {
    scroller.scrollLeft += 12;
  }
}

function openCategoryMenu(category: Category, event: MouseEvent) {
  event.preventDefault();
  event.stopPropagation();
  pendingDeleteCategoryId.value = null;
  editingColorCategoryId.value = null;
  categoryMenuAnchorRect.value = (event.currentTarget as HTMLElement).getBoundingClientRect();
  const menuWidth = 168;
  const padding = 8;
  categoryMenu.value = {
    category,
    left: Math.min(Math.max(event.clientX, padding), window.innerWidth - menuWidth - padding),
    top: Math.min(Math.max(event.clientY, padding), window.innerHeight - 180),
  };
}

function closeCategoryMenu() {
  categoryMenu.value = null;
}

function closeFloatingLayers() {
  pendingDeleteCategoryId.value = null;
  editingColorCategoryId.value = null;
  categoryMenuAnchorRect.value = null;
  categoryMenu.value = null;
}

defineExpose({
  closeFloatingLayers,
});

function categoryDragStyle(category: Category) {
  if (draggingCategoryId.value !== category.id) return undefined;
  const state = categoryDragState;
  return {
    transform: props.orientation === "vertical"
      ? `translate3d(0, ${categoryDragOffset.value.y}px, 0)`
      : `translate3d(${categoryDragOffset.value.x}px, 0, 0)`,
    width: state?.width ? `${state.width}px` : undefined,
    height: state?.height ? `${state.height}px` : undefined,
  };
}

function countLabel(count: number | undefined) {
  return (count ?? 0) > 99 ? "99+" : String(Math.max(count ?? 0, 0));
}
</script>

<template>
  <section
    ref="railElement"
    class="tag-strip"
    :class="{ 'tag-strip-vertical': orientation === 'vertical' }"
  >
    <nav
      ref="categoryScroller"
      class="category-scroll subtle-scrollbar min-w-0 flex-1 overflow-x-auto py-1"
      :class="{
        'category-scroll-vertical': orientation === 'vertical',
        'subtle-scrollbar-active': isCategoryScrolling,
      }"
      @wheel="handleCategoryWheel"
      @scroll="showCategoryScrollbar"
      @mouseenter="activateCategoryScrollbar"
      @mouseleave="scheduleHideCategoryScrollbar"
    >
      <button
        type="button"
        class="category-chip"
        :class="{ 'category-chip-active': selectedCategoryId === 'history' }"
        tabindex="-1"
        @click="selectCategory('history')"
      >
        <span class="category-count-dot category-count-dot-history">{{ countLabel(historyCount) }}</span>
        <span class="category-chip-label">{{ t("category.history") }}</span>
      </button>

      <div
        v-for="category in categories"
        :key="category.id"
        :data-category-id="category.id"
        class="category-chip category-chip-group group"
        :class="{
          'category-chip-active': selectedCategoryId === category.id,
          'category-chip-dragging': draggingCategoryId === category.id,
          'category-chip-drop-before': categoryDropTargetId === category.id && categoryDropSide === 'before',
          'category-chip-drop-after': categoryDropTargetId === category.id && categoryDropSide === 'after',
        }"
        :style="categoryDragStyle(category)"
        @click="selectCategory(category.id)"
        @dblclick.stop="editCategory(category)"
        @contextmenu="openCategoryMenu(category, $event)"
        @pointerdown="startCategoryDrag(category, $event)"
      >
        <span
          v-if="editingCategoryId !== category.id"
          class="category-color-dot category-count-dot"
          :style="{ backgroundColor: category.color }"
        >
          {{ countLabel(categoryCounts[category.id]) }}
        </span>
        <span
          v-if="editingCategoryId !== category.id"
          class="category-chip-label"
        >
          {{ categoryDisplayName(category.name) }}
        </span>
        <input
          v-else
          v-model="editingName"
          class="category-chip-input"
          tabindex="-1"
          @click.stop
          @keydown.enter.prevent.stop="commitEditing(category)"
          @keydown.escape.prevent.stop="emit('finishEditing')"
          @blur="commitEditing(category)"
        />
        <div
          v-if="editingColorCategoryId === category.id"
          class="category-color-popover"
          :style="{ left: `${colorPopoverPosition.left}px`, top: `${colorPopoverPosition.top}px` }"
          @click.stop
          @pointerdown.stop
          @mouseleave="closeColorPicker"
        >
          <div class="category-color-popover-title">
            <Palette class="size-3.5" />
            <span>{{ t("category.color") }}</span>
          </div>
          <div class="category-color-grid">
            <button
              v-for="color in CATEGORY_COLOR_OPTIONS"
              :key="color"
              type="button"
              class="category-color-swatch"
              :class="{ 'category-color-swatch-active': color.toLowerCase() === category.color.toLowerCase() }"
              :style="{ backgroundColor: color }"
              :aria-label="t('category.selectColor', { color })"
              tabindex="-1"
              @click="updateColor(category, color)"
            >
              <Check v-if="color.toLowerCase() === category.color.toLowerCase()" class="size-3.5" />
            </button>
          </div>
          <label class="category-custom-color">
            <input
              type="color"
              :value="category.color"
              tabindex="-1"
              @change="updateColor(category, ($event.target as HTMLInputElement).value)"
            />
            <span>{{ t("category.customColor") }}</span>
          </label>
        </div>
      </div>
    </nav>

    <div class="category-create-wrap flex shrink-0 items-center gap-2">
      <button type="button" class="category-chip category-chip-create" tabindex="-1" @click="emit('create')">
        <Plus class="size-4" />
        <span>{{ t("category.create") }}</span>
      </button>
    </div>

    <div
      v-if="categoryMenu"
      class="category-context-menu"
      :style="{ left: `${categoryMenu.left}px`, top: `${categoryMenu.top}px` }"
      @click.stop
      @contextmenu.prevent.stop
    >
      <button type="button" class="category-context-item" tabindex="-1" @click="editCategory(categoryMenu.category)">
        <Pencil class="size-3.5" />
        <span>{{ t("common.rename") }}</span>
      </button>
      <button type="button" class="category-context-item" tabindex="-1" @click="openColorPicker(categoryMenu.category, $event)">
        <Palette class="size-3.5" />
        <span>{{ t("category.changeColor") }}</span>
      </button>
      <div class="context-menu-separator" />
      <button
        type="button"
        class="category-context-item category-context-item-danger"
        :class="{ 'category-context-item-confirm': pendingDeleteCategoryId === categoryMenu.category.id }"
        tabindex="-1"
        @click="requestDeleteCategory(categoryMenu.category.id, { keepMenuOpen: true })"
      >
        <Trash2 class="size-3.5" />
        <span>{{ pendingDeleteCategoryId === categoryMenu.category.id ? t("common.confirmDelete") : t("category.delete") }}</span>
      </button>
    </div>
  </section>
</template>
