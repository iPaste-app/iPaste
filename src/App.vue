<script setup lang="ts">
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { AlertCircle, ChevronRight, ClipboardCopy, CornerDownLeft, FolderInput, Inbox, Pencil, Plus, Trash2, X } from "lucide-vue-next";
import CategoryRail from "./components/CategoryRail.vue";
import ClipCard from "./components/ClipCard.vue";
import ClipViewerWindow from "./components/ClipViewerWindow.vue";
import SettingsWindow from "./components/SettingsWindow.vue";
import TopBar from "./components/TopBar.vue";
import UpdateDialog from "./components/UpdateDialog.vue";
import { useUpdater } from "./composables/useUpdater";
import { t } from "./i18n";
import { clipImageSrc } from "./lib/clipMedia";
import { categoryDisplayName, clipMetricText, formatShortcut, formatTime, typeLabel } from "./lib/format";
import { ipasteApi } from "./lib/ipasteApi";
import { useIpasteStore } from "./stores/ipasteStore";
import type { Category, ClipViewItem } from "./types";

const CATEGORY_COLORS = ["#0D9488", "#2563EB", "#7C3AED", "#D97706", "#DC2626", "#475569"];
const store = useIpasteStore();
const updater = useUpdater();
const isSettingsWindow = new URLSearchParams(window.location.search).get("window") === "settings";
const isClipViewerWindow = new URLSearchParams(window.location.search).get("window") === "clip-viewer";
const isMacOs = /mac/i.test(navigator.platform) || /Mac OS/i.test(navigator.userAgent);
const isPreservingCurrentApp = ref(false);
const contextMenu = ref<{ item: ClipViewItem; index: number; x: number; y: number } | null>(null);
const contextMenuElement = ref<HTMLElement | null>(null);
const moveSubmenuBranchElement = ref<HTMLElement | null>(null);
const moveSubmenuElement = ref<HTMLElement | null>(null);
const showMoveSubmenu = ref(false);
const submenuAlignLeft = ref(false);
const submenuOffsetTop = ref(0);
const editingCategoryId = ref<string | null>(null);
const categoryRailElement = ref<InstanceType<typeof CategoryRail> | null>(null);
const clipListElement = ref<HTMLElement | null>(null);
const hoveredPreviewItemKey = ref<string | null>(null);
const lockedPreviewItemKey = ref<string | null>(null);
const isQuickPreviewPinned = ref(false);
const isQuickPreviewKeyDown = ref(false);
const isQuickPreviewActive = ref(false);
const quickPreviewSelectedText = ref("");
const pendingDeleteContextKey = ref<string | null>(null);
const editingClipKey = ref<string | null>(null);
const editingClipName = ref("");
const isClipListScrolling = ref(false);
const draggingItemKey = ref<string | null>(null);
const itemDropTargetKey = ref<string | null>(null);
const itemDropSide = ref<"before" | "after" | null>(null);
const itemDragOffset = ref({ x: 0, y: 0 });
const isTauri = "__TAURI_INTERNALS__" in window;
let itemDragState: {
  key: string;
  id: string;
  startX: number;
  startY: number;
  width: number;
  height: number;
  hasMoved: boolean;
  targetKey: string | null;
  targetId: string | null;
  side: "before" | "after" | null;
} | null = null;
let unlistenShortcutOpened: UnlistenFn | null = null;
let unlistenPanelVisibilityChanged: UnlistenFn | null = null;
let unlistenPanelKey: UnlistenFn | null = null;
let moveSubmenuCloseTimer: number | null = null;
let clipListScrollTimer: number | null = null;
let selectionScrollFrame: number | null = null;
let searchReloadTimer: number | null = null;
let quickPreviewOpenTimer: number | null = null;
let lastUpdateCheckAt = 0;
let suppressNextItemSelect = false;
let suppressQuickPreviewUntilModifierUp = false;

const categoryById = computed(() =>
  store.categories.reduce<Record<string, Category>>((categories, category) => {
    categories[category.id] = category;
    return categories;
  }, {}),
);

const categoriesByHash = computed(() =>
  store.categoryItems.reduce<Record<string, Category[]>>((groups, item) => {
    const category = categoryById.value[item.categoryId];
    if (!category) return groups;

    groups[item.contentHash] = [...(groups[item.contentHash] ?? []), category];
    return groups;
  }, {}),
);

const categoryItemCounts = computed(() =>
  store.categoryItems.reduce<Record<string, number>>((counts, item) => {
    counts[item.categoryId] = (counts[item.categoryId] ?? 0) + 1;
    return counts;
  }, {}),
);

const formattedShortcut = computed(() => `${formatShortcut("CommandOrControl+F")} ${t("shortcut.search")}`);
const isSideLayout = computed(() => store.panelLayout === "side");
const canReorderVisibleItems = computed(() =>
  store.selectedCategoryId !== "history" && !store.search.trim() && store.visibleItems.length > 1,
);
const quickPreviewItem = computed(() => {
  if (!isQuickPreviewActive.value || contextMenu.value || editingClipKey.value) return null;

  const itemKey = lockedPreviewItemKey.value ?? hoveredPreviewItemKey.value;
  if (!itemKey) return null;
  return store.visibleItems.find((item) => contextItemKey(item) === itemKey) ?? null;
});
const isQuickPreviewLocked = computed(() => isQuickPreviewPinned.value);
const quickPreviewTitle = computed(() => {
  const item = quickPreviewItem.value;
  if (!item) return "";
  return item.displayName?.trim() || "";
});
const quickPreviewAriaLabel = computed(() => {
  const item = quickPreviewItem.value;
  if (!item) return "";
  return item.displayName?.trim() || t("clip.clipboardTitle", { type: typeLabel(item.clipType) });
});
const quickPreviewContent = computed(() => quickPreviewItem.value?.text || quickPreviewItem.value?.previewText || "");
const quickPreviewImageSrc = computed(() => quickPreviewItem.value ? clipImageSrc(quickPreviewItem.value) : "");
const quickPreviewTime = computed(() => {
  const item = quickPreviewItem.value;
  if (!item) return "";
  return item.collection === "history" ? item.lastCapturedAt : item.createdAt;
});
const quickPreviewSize = computed(() => {
  const item = quickPreviewItem.value;
  if (!item) return "";
  return clipMetricText(item.clipType, item.text, item.previewText);
});
const quickPreviewColorValue = computed(() => quickPreviewContent.value.trim());

onMounted(async () => {
  if (isClipViewerWindow) return;
  if (isSettingsWindow) return;

  document.addEventListener("keydown", handleKeydown, true);
  document.addEventListener("keyup", handleKeyup, true);
  document.addEventListener("selectionchange", handleSelectionChange);
  window.addEventListener("blur", closeFloatingLayers);
  document.addEventListener("visibilitychange", handleVisibilityChange);

  await store.load();
  await store.bindEvents();
  if (isTauri) {
    scheduleSilentUpdateCheck();
  }
  if (isTauri) {
    unlistenShortcutOpened = await listen("ipaste://shortcut-opened", closeFloatingLayers);
    unlistenPanelKey = await listen<{ key: PanelKey }>("ipaste://panel-key", (event) => {
      handlePanelKey(event.payload.key);
    });
    unlistenPanelVisibilityChanged = await listen<{ visible: boolean; preservesCurrentApp: boolean; nativePanel?: boolean }>(
      "ipaste://panel-visibility-changed",
      (event) => {
        applyPanelVisibility(event.payload);
      },
    );
  }
});

onUnmounted(() => {
  if (isClipViewerWindow) return;
  if (isSettingsWindow) return;

  document.removeEventListener("keydown", handleKeydown, true);
  document.removeEventListener("keyup", handleKeyup, true);
  document.removeEventListener("selectionchange", handleSelectionChange);
  window.removeEventListener("blur", closeFloatingLayers);
  document.removeEventListener("visibilitychange", handleVisibilityChange);
  clearMoveSubmenuCloseTimer();
  clearClipListScrollTimer();
  clearSelectionScrollFrame();
  clearSearchReloadTimer();
  clearQuickPreviewTimer();
  cleanupItemDrag();
  unlistenShortcutOpened?.();
  unlistenPanelKey?.();
  unlistenPanelVisibilityChanged?.();
  unlistenShortcutOpened = null;
  unlistenPanelKey = null;
  unlistenPanelVisibilityChanged = null;
  document.body.classList.remove("ipaste-preserve-current-app");
});

watch(
  () => store.search,
  () => {
    store.clampSelection();
    if (store.selectedCategoryId === "history") {
      scheduleSearchReload();
    }
  },
);

watch(
  () => [store.selectedIndex, store.selectedCategoryId, store.search],
  () => scheduleSelectedClipScroll(),
  { flush: "post" },
);

watch(isPreservingCurrentApp, (preservesCurrentApp) => {
  document.body.classList.toggle("ipaste-preserve-current-app", preservesCurrentApp);
});

function applyPanelVisibility(
  payload: { visible: boolean; preservesCurrentApp: boolean; nativePanel?: boolean },
  activateDefault = false,
) {
  closeFloatingLayers();
  const nativePanel = payload.visible && Boolean(payload.nativePanel);
  isPreservingCurrentApp.value = payload.visible && payload.preservesCurrentApp && !nativePanel;
  if (!payload.visible) {
    store.clearSearch();
    resetClipListScroll();
    blurActiveElement();
    return;
  }

  if (activateDefault) {
    store.activatePanelDefault();
  }
  if (!nativePanel) {
    scheduleActiveElementBlur();
  }
  blurCategoryFocus();
  scheduleSilentUpdateCheck();
}

async function createCategory() {
  const category = await store.createCategory(t("category.newCategory"));
  editingCategoryId.value = category.id;
}

async function renameCategory(category: Category, name: string) {
  if (!name || name === category.name) return;
  await store.renameCategory(category, name);
}

async function updateCategoryColor(category: Category, color: string) {
  await store.updateCategoryColor(category, color);
}

async function editCategory(id: string) {
  editingCategoryId.value = id;
}

function finishEditingCategory() {
  editingCategoryId.value = null;
}

async function createCategoryForContextItem() {
  const item = contextMenu.value?.item;
  if (!item) return;

  closeFloatingLayers();
  const clipId = item.collection === "history" ? item.id : item.clipSnapshotId;
  const color = CATEGORY_COLORS[store.categories.length % CATEGORY_COLORS.length];
  const { category, item: categoryItem } = await ipasteApi.createCategoryWithClip(t("category.newCategory"), color, clipId);
  store.categories.push(category);
  store.categoryItems.push(categoryItem);
  store.clips = store.clips.map((clip) =>
    clip.id === clipId ? { ...clip, favoriteCount: clip.favoriteCount + 1 } : clip,
  );
  store.selectCategory(category.id);
  store.syncCloudInBackground();
  editingCategoryId.value = category.id;
}

async function deleteCategory(id: string) {
  await store.deleteCategory(id);
}

async function reorderCategories(categoryIds: string[]) {
  await store.reorderCategories(categoryIds);
}

function itemCategoryTags(item: ClipViewItem) {
  if (item.collection === "history") return categoriesByHash.value[item.contentHash] ?? [];

  const categoryId = "categoryId" in item ? item.categoryId : store.selectedCategoryId;
  const category = categoryById.value[categoryId];
  return category ? [category] : [];
}

function openClipContextMenu(payload: { item: ClipViewItem; index: number; x: number; y: number }) {
  store.setSelectedIndex(payload.index);
  pendingDeleteContextKey.value = null;
  contextMenu.value = payload;
  void nextTick(positionContextMenu);
}

function positionContextMenu() {
  if (!contextMenu.value || !contextMenuElement.value) return;

  const rect = contextMenuElement.value.getBoundingClientRect();
  const padding = 8;
  const maxX = Math.max(padding, window.innerWidth - rect.width - padding);
  const maxY = Math.max(padding, window.innerHeight - rect.height - padding);
  contextMenu.value = {
    ...contextMenu.value,
    x: clamp(contextMenu.value.x, padding, maxX),
    y: clamp(contextMenu.value.y, padding, maxY),
  };
  positionMoveSubmenu();
}

async function pasteContextItem() {
  const item = contextMenu.value?.item;
  closeFloatingLayers();
  if (!item) return;
  await store.applyItem(item);
}

async function copyContextItem() {
  const item = contextMenu.value?.item;
  closeFloatingLayers();
  if (!item) return;
  await store.copyItem(item);
}

async function renameContextItem() {
  const item = contextMenu.value?.item;
  closeFloatingLayers();
  if (!item) return;

  await startEditingClipName(item);
}

async function addContextItemToCategory(categoryId: string) {
  const item = contextMenu.value?.item;
  if (!item) return;
  closeFloatingLayers();
  await addItemToCategory(item, categoryId);
}

async function addItemToCategory(item: ClipViewItem, categoryId: string) {
  const clipId = item.collection === "history" ? item.id : item.clipSnapshotId;
  await store.addToCategory(clipId, categoryId);
}

function startItemDrag(payload: { item: ClipViewItem; index: number; event: PointerEvent }) {
  if (!canReorderVisibleItems.value || payload.item.collection !== "category" || payload.event.button !== 0) {
    payload.event.preventDefault();
    return;
  }

  payload.event.preventDefault();
  const key = contextItemKey(payload.item);
  const dragSource = (payload.event.currentTarget ?? payload.event.target) as Element | null;
  const card = dragSource?.closest<HTMLElement>("[data-item-key]");
  const rect = card?.getBoundingClientRect();
  pendingDeleteContextKey.value = null;
  closeMoveSubmenu();
  itemDragState = {
    key,
    id: payload.item.id,
    startX: payload.event.clientX,
    startY: payload.event.clientY,
    width: rect?.width ?? 0,
    height: rect?.height ?? 0,
    hasMoved: false,
    targetKey: null,
    targetId: null,
    side: null,
  };
  itemDragOffset.value = { x: 0, y: 0 };
  window.addEventListener("pointermove", handleItemPointerMove);
  window.addEventListener("pointerup", finishItemDrag);
  window.addEventListener("pointercancel", cancelItemDrag);
}

function handleItemPointerMove(event: PointerEvent) {
  const state = itemDragState;
  if (!state || !canReorderVisibleItems.value) return;

  event.preventDefault();
  if (Math.hypot(event.clientX - state.startX, event.clientY - state.startY) > 3) {
    if (!state.hasMoved) {
      draggingItemKey.value = state.key;
    }
    state.hasMoved = true;
  }
  if (!state.hasMoved) return;

  itemDragOffset.value = {
    x: event.clientX - state.startX,
    y: event.clientY - state.startY,
  };

  const target = itemTargetFromPoint(event.clientX, event.clientY);
  if (!target || target.key === state.key) {
    state.targetKey = null;
    state.targetId = null;
    state.side = null;
    itemDropTargetKey.value = null;
    itemDropSide.value = null;
    return;
  }

  const side = event.clientY < target.rect.top + target.rect.height / 2 ? "before" : "after";
  state.targetKey = target.key;
  state.targetId = target.id;
  state.side = side;
  itemDropTargetKey.value = target.key;
  itemDropSide.value = side;
  scrollItemsNearPointer(event.clientY);
  showClipListScrollbar();
}

async function finishItemDrag(event?: PointerEvent) {
  event?.preventDefault();

  const state = itemDragState;
  if (state?.hasMoved) {
    suppressNextItemSelect = true;
    window.setTimeout(() => {
      suppressNextItemSelect = false;
    }, 0);
  }
  cleanupItemDrag();
  if (!state?.hasMoved || !state.targetKey || !state.targetId || !state.side || state.key === state.targetKey) return;
  const currentItems = store.visibleItems.filter((item) => item.collection === "category");
  const draggedItem = currentItems.find((item) => contextItemKey(item) === state.key);
  if (!draggedItem) return;

  const nextIds = currentItems
    .filter((item) => contextItemKey(item) !== state.key)
    .map((item) => item.id);
  const targetIndex = nextIds.indexOf(state.targetId);
  if (targetIndex < 0) return;

  nextIds.splice(state.side === "after" ? targetIndex + 1 : targetIndex, 0, draggedItem.id);
  const currentIds = currentItems.map((item) => item.id);
  if (nextIds.join("\n") === currentIds.join("\n")) return;

  await store.reorderCategoryItems(store.selectedCategoryId, nextIds);
}

function cancelItemDrag() {
  cleanupItemDrag();
}

function cleanupItemDrag() {
  window.removeEventListener("pointermove", handleItemPointerMove);
  window.removeEventListener("pointerup", finishItemDrag);
  window.removeEventListener("pointercancel", cancelItemDrag);
  itemDragState = null;
  draggingItemKey.value = null;
  itemDropTargetKey.value = null;
  itemDropSide.value = null;
  itemDragOffset.value = { x: 0, y: 0 };
}

function itemTargetFromPoint(clientX: number, clientY: number) {
  const element = document.elementFromPoint(clientX, clientY);
  const card = element instanceof Element ? element.closest<HTMLElement>("[data-item-key]") : null;
  if (!card || !clipListElement.value?.contains(card)) return null;

  const key = card.dataset.itemKey;
  const id = card.dataset.itemId;
  if (!key || !id) return null;
  return {
    key,
    id,
    rect: card.getBoundingClientRect(),
  };
}

function scrollItemsNearPointer(clientY: number) {
  const list = clipListElement.value;
  if (!list) return;

  const rect = list.getBoundingClientRect();
  const edge = 48;
  if (clientY < rect.top + edge) {
    list.scrollTop -= 14;
  } else if (clientY > rect.bottom - edge) {
    list.scrollTop += 14;
  }
}

function itemDragStyle(item: ClipViewItem) {
  if (draggingItemKey.value !== contextItemKey(item)) return undefined;
  const state = itemDragState;
  return {
    transform: `translate(${itemDragOffset.value.x}px, ${itemDragOffset.value.y}px)`,
    width: state?.width ? `${state.width}px` : undefined,
    height: state?.height ? `${state.height}px` : undefined,
  };
}

function selectClipCard(index: number) {
  if (suppressNextItemSelect) {
    suppressNextItemSelect = false;
    return;
  }

  store.setSelectedIndex(index);
}

function hoverPreviewItem(item: ClipViewItem) {
  if (isQuickPreviewActive.value) return;

  hoveredPreviewItemKey.value = contextItemKey(item);
  if (isQuickPreviewKeyDown.value && !suppressQuickPreviewUntilModifierUp) {
    scheduleQuickPreview();
  }
}

function clearHoveredPreviewItem(item: ClipViewItem) {
  if (isQuickPreviewKeyDown.value) return;

  if (hoveredPreviewItemKey.value === contextItemKey(item)) {
    hoveredPreviewItemKey.value = null;
  }
  stopQuickPreview();
}

function clearQuickPreviewHover() {
  if (isQuickPreviewActive.value) return;

  hoveredPreviewItemKey.value = null;
  stopQuickPreview();
}

function scheduleQuickPreview() {
  if (!hoveredPreviewItemKey.value || contextMenu.value || isEditableTarget(document.activeElement)) return;

  clearQuickPreviewTimer();
  const previewItemKey = hoveredPreviewItemKey.value;
  quickPreviewOpenTimer = window.setTimeout(() => {
    quickPreviewOpenTimer = null;
    if (isQuickPreviewKeyDown.value && hoveredPreviewItemKey.value && !suppressQuickPreviewUntilModifierUp) {
      lockedPreviewItemKey.value = previewItemKey;
      isQuickPreviewActive.value = true;
    }
  }, 140);
}

function stopQuickPreview(options: { force?: boolean } = {}) {
  clearQuickPreviewTimer();
  if (isQuickPreviewPinned.value && !options.force) return;

  lockedPreviewItemKey.value = null;
  isQuickPreviewPinned.value = false;
  isQuickPreviewActive.value = false;
}

function lockQuickPreview() {
  const item = quickPreviewItem.value;
  if (!item) return;
  lockedPreviewItemKey.value = contextItemKey(item);
  isQuickPreviewPinned.value = true;
  isQuickPreviewActive.value = true;
}

function closeQuickPreview() {
  lockedPreviewItemKey.value = null;
  isQuickPreviewPinned.value = false;
  isQuickPreviewKeyDown.value = false;
  suppressQuickPreviewUntilModifierUp = false;
  quickPreviewSelectedText.value = "";
  window.getSelection()?.removeAllRanges();
  stopQuickPreview({ force: true });
}

async function copyQuickPreviewItem() {
  const item = quickPreviewItem.value;
  if (!item) return;
  await store.copyItem(item);
}

async function pasteQuickPreviewSelection() {
  const item = quickPreviewItem.value;
  const selectedText = quickPreviewSelectedText.value.trim();
  if (!item || !selectedText) return;

  await ipasteApi.applyClip(originalClipId(item), item.clipType, selectedText);
  closeQuickPreview();
}

function handleSelectionChange() {
  if (!quickPreviewItem.value) {
    quickPreviewSelectedText.value = "";
    return;
  }

  const selection = window.getSelection();
  const text = selection?.toString() ?? "";
  const anchorNode = selection?.anchorNode;
  const focusNode = selection?.focusNode;
  const previewElement = clipListElement.value?.parentElement?.querySelector(".quick-preview-overlay");
  const selectionInPreview = Boolean(
    previewElement
      && anchorNode
      && focusNode
      && previewElement.contains(anchorNode)
      && previewElement.contains(focusNode),
  );

  quickPreviewSelectedText.value = selectionInPreview ? text : "";
}

function clearQuickPreviewTimer() {
  if (quickPreviewOpenTimer === null) return;
  window.clearTimeout(quickPreviewOpenTimer);
  quickPreviewOpenTimer = null;
}

async function deleteContextItem() {
  const item = contextMenu.value?.item;
  if (!item) return;

  const deleteKey = contextItemKey(item);
  if (pendingDeleteContextKey.value !== deleteKey) {
    pendingDeleteContextKey.value = deleteKey;
    return;
  }

  closeFloatingLayers();
  if (item.collection === "history") {
    await store.deleteClip(item.id);
    return;
  }

  await store.removeCategoryItem(item.id);
}

function contextItemKey(item: ClipViewItem) {
  return `${item.collection}-${item.id}`;
}

function originalClipId(item: ClipViewItem) {
  return item.collection === "history" ? item.id : item.clipSnapshotId;
}

function contextDeleteLabel(item: ClipViewItem) {
  const isPending = pendingDeleteContextKey.value === contextItemKey(item);
  if (item.collection === "history") return isPending ? t("common.confirmDelete") : t("common.delete");
  return isPending ? t("context.confirmRemove") : t("context.removeFromCategory");
}

async function startEditingClipName(item: ClipViewItem) {
  const index = store.visibleItems.findIndex((visibleItem) => contextItemKey(visibleItem) === contextItemKey(item));
  if (index >= 0) {
    store.setSelectedIndex(index);
  }

  editingClipKey.value = contextItemKey(item);
  editingClipName.value = item.displayName?.trim() || typeLabel(item.clipType);
  await focusEditingClipName();
}

function updateEditingClipName(value: string) {
  editingClipName.value = value;
}

async function commitEditingClipName(item: ClipViewItem) {
  if (editingClipKey.value !== contextItemKey(item)) return;

  const name = editingClipName.value.trim();
  editingClipKey.value = null;
  editingClipName.value = "";
  await store.renameClip(item, name || null);
}

function cancelEditingClipName() {
  editingClipKey.value = null;
  editingClipName.value = "";
}

async function openClipViewer(item: ClipViewItem) {
  await ipasteApi.openClipViewer(item, originalClipId(item));
}

function handleKeydown(event: KeyboardEvent) {
  if (event.defaultPrevented) return;

  if (quickPreviewItem.value) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeQuickPreview();
    }
    if (event.key === "Enter" && quickPreviewSelectedText.value.trim()) {
      event.preventDefault();
      void pasteQuickPreviewSelection();
    }
    return;
  }

  if (isQuickPreviewModifierKey(event)) {
    isQuickPreviewKeyDown.value = true;
    suppressQuickPreviewUntilModifierUp = false;
    scheduleQuickPreview();
  } else if (hasQuickPreviewModifier(event)) {
    suppressQuickPreviewUntilModifierUp = true;
    stopQuickPreview();
  }

  if (handleCategoryShortcut(event)) return;

  if (event.key === "Tab") {
    event.preventDefault();
    return;
  }

  if (isEditableTarget(event.target)) return;

  if (contextMenu.value) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeFloatingLayers();
    }
    return;
  }

  if (handlePanelKey(event.key)) {
    event.preventDefault();
    return;
  }

  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "f") {
    event.preventDefault();
    focusSearch();
  }
}

function handleKeyup(event: KeyboardEvent) {
  if (!isQuickPreviewModifierKey(event)) return;

  isQuickPreviewKeyDown.value = false;
  suppressQuickPreviewUntilModifierUp = false;
  stopQuickPreview();
}

function isQuickPreviewModifierKey(event: KeyboardEvent) {
  return isMacOs ? event.key === "Meta" || event.key === "Command" : event.key === "Control";
}

function hasQuickPreviewModifier(event: KeyboardEvent) {
  return isMacOs ? event.metaKey : event.ctrlKey;
}

type PanelKey = "ArrowDown" | "ArrowUp" | "ArrowRight" | "ArrowLeft" | "Enter" | "Escape";

function handlePanelKey(key: string) {
  if (contextMenu.value) {
    if (key === "Escape") {
      closeFloatingLayers();
      return true;
    }
    return false;
  }

  if (key === "ArrowDown") {
    store.moveSelection(2);
    return true;
  }

  if (key === "ArrowUp") {
    store.moveSelection(-2);
    return true;
  }

  if (key === "ArrowRight") {
    store.moveSelection(1);
    return true;
  }

  if (key === "ArrowLeft") {
    store.moveSelection(-1);
    return true;
  }

  if (key === "Enter") {
    void store.applySelected();
    return true;
  }

  if (key === "Escape") {
    void hidePanelFromUi();
    return true;
  }

  return false;
}

function handleCategoryShortcut(event: KeyboardEvent) {
  if (!(event.metaKey || event.ctrlKey) || event.altKey || !/^[1-9]$/.test(event.key)) {
    return false;
  }

  event.preventDefault();

  const categoryIds = ["history", ...store.categories.map((category) => category.id)];
  const targetCategoryId = categoryIds[Number(event.key) - 1];
  if (!targetCategoryId) return true;

  closeFloatingLayers();
  finishEditingCategory();
  if (targetCategoryId !== store.selectedCategoryId) {
    store.selectCategory(targetCategoryId);
  }
  return true;
}

function isEditableTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false;
  return Boolean(target.closest("input, textarea, select, [contenteditable='true']"));
}

function focusSearch() {
  const input = document.querySelector<HTMLInputElement>(".search-box input");
  input?.focus();
  input?.select();
}

async function hidePanelFromUi() {
  blurActiveElement();
  await store.hidePanel();
}

function scheduleActiveElementBlur() {
  void nextTick(() => {
    window.requestAnimationFrame(blurActiveElement);
  });
}

function blurActiveElement() {
  const activeElement = document.activeElement;
  if (activeElement instanceof HTMLElement && activeElement !== document.body) {
    activeElement.blur();
  }
}

function blurCategoryFocus() {
  window.requestAnimationFrame(() => {
    const activeElement = document.activeElement;
    if (activeElement instanceof HTMLElement && activeElement.closest(".tag-strip")) {
      activeElement.blur();
    }
  });
}

function closeFloatingLayers() {
  contextMenu.value = null;
  pendingDeleteContextKey.value = null;
  hoveredPreviewItemKey.value = null;
  isQuickPreviewKeyDown.value = false;
  suppressQuickPreviewUntilModifierUp = false;
  stopQuickPreview({ force: true });
  closeMoveSubmenu();
  categoryRailElement.value?.closeFloatingLayers();
}

function handleVisibilityChange() {
  if (document.hidden) {
    closeFloatingLayers();
  } else {
    scheduleSilentUpdateCheck();
  }
}

function scheduleSilentUpdateCheck() {
  if (!isTauri) return;

  const now = Date.now();
  if (now - lastUpdateCheckAt < 30 * 60 * 1000) return;
  lastUpdateCheckAt = now;
  void updater.checkForUpdate({ silent: true });
}

async function focusEditingClipName() {
  await nextTick();
  window.setTimeout(() => {
    const input = document.querySelector<HTMLInputElement>(".clip-title-input");
    input?.focus();
    input?.select();
  }, 40);
}

async function openMoveSubmenu() {
  clearMoveSubmenuCloseTimer();
  showMoveSubmenu.value = true;
  await nextTick();
  positionMoveSubmenu();
}

function scheduleCloseMoveSubmenu() {
  clearMoveSubmenuCloseTimer();
  moveSubmenuCloseTimer = window.setTimeout(() => {
    showMoveSubmenu.value = false;
    moveSubmenuCloseTimer = null;
  }, 120);
}

function closeMoveSubmenu() {
  clearMoveSubmenuCloseTimer();
  showMoveSubmenu.value = false;
  submenuOffsetTop.value = 0;
}

function clearMoveSubmenuCloseTimer() {
  if (moveSubmenuCloseTimer === null) return;
  window.clearTimeout(moveSubmenuCloseTimer);
  moveSubmenuCloseTimer = null;
}

function showClipListScrollbar() {
  clearClipListScrollTimer();
  isClipListScrolling.value = true;
  clipListScrollTimer = window.setTimeout(() => {
    isClipListScrolling.value = false;
    clipListScrollTimer = null;
  }, 780);
}

function handleClipListScroll() {
  showClipListScrollbar();

  const list = clipListElement.value;
  if (!list || store.selectedCategoryId !== "history" || !store.hasMoreClips) return;

  const distanceToBottom = list.scrollHeight - list.scrollTop - list.clientHeight;
  if (distanceToBottom < 160) {
    void store.loadMoreClips();
  }
}

function clearClipListScrollTimer() {
  if (clipListScrollTimer === null) return;
  window.clearTimeout(clipListScrollTimer);
  clipListScrollTimer = null;
}

function resetClipListScroll() {
  clearClipListScrollTimer();
  isClipListScrolling.value = false;

  if (clipListElement.value) {
    clipListElement.value.scrollTop = 0;
  }
}

function positionMoveSubmenu() {
  if (!moveSubmenuBranchElement.value || !moveSubmenuElement.value) return;

  const branchRect = moveSubmenuBranchElement.value.getBoundingClientRect();
  const submenuRect = moveSubmenuElement.value.getBoundingClientRect();
  const padding = 8;
  const maxY = Math.max(padding, window.innerHeight - submenuRect.height - padding);

  submenuAlignLeft.value = branchRect.right + submenuRect.width + padding > window.innerWidth
    && branchRect.left - submenuRect.width - padding >= padding;
  submenuOffsetTop.value = clamp(branchRect.top, padding, maxY) - branchRect.top;
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

function scheduleSelectedClipScroll() {
  clearSelectionScrollFrame();
  selectionScrollFrame = window.requestAnimationFrame(() => {
    selectionScrollFrame = null;
    scrollSelectedClipIntoView();
  });
}

function clearSelectionScrollFrame() {
  if (selectionScrollFrame === null) return;
  window.cancelAnimationFrame(selectionScrollFrame);
  selectionScrollFrame = null;
}

function scheduleSearchReload() {
  clearSearchReloadTimer();
  searchReloadTimer = window.setTimeout(() => {
    searchReloadTimer = null;
    void store.reloadClips();
  }, 160);
}

function clearSearchReloadTimer() {
  if (searchReloadTimer === null) return;
  window.clearTimeout(searchReloadTimer);
  searchReloadTimer = null;
}

function scrollSelectedClipIntoView() {
  const list = clipListElement.value;
  const selectedCard = list?.querySelector<HTMLElement>(".clip-card-selected");
  if (!list || !selectedCard) return;

  const listRect = list.getBoundingClientRect();
  const cardRect = selectedCard.getBoundingClientRect();
  const edgePadding = 16;
  const visibleTop = listRect.top + edgePadding;
  const visibleBottom = listRect.bottom - edgePadding;

  if (cardRect.top < visibleTop) {
    list.scrollBy({ top: cardRect.top - visibleTop, behavior: "auto" });
    showClipListScrollbar();
    return;
  }

  if (cardRect.bottom > visibleBottom) {
    list.scrollBy({ top: cardRect.bottom - visibleBottom, behavior: "auto" });
    showClipListScrollbar();
  }
}
</script>

<template>
  <SettingsWindow v-if="isSettingsWindow" />
  <ClipViewerWindow v-else-if="isClipViewerWindow" />

  <main
    v-else
    class="app-shell"
    :class="{ 'app-shell-preserve-current-app': isPreservingCurrentApp }"
    @click="closeFloatingLayers"
  >
    <section class="flex min-w-0 flex-1 flex-col">
      <div class="relative">
        <TopBar
          v-model="store.search"
          :shortcut="formattedShortcut"
          :settings-open="false"
          :append-copy-enabled="store.isAppendCopyEnabled"
          :append-copy-timeout-minutes="store.appendCopyTimeoutMinutes"
          :has-update="updater.hasAvailableUpdate.value"
          @toggle-settings="store.showSettings"
          @toggle-append-copy="store.toggleAppendCopy"
          @open-update="updater.openUpdateDialog"
          @close="hidePanelFromUi"
        />
      </div>

      <UpdateDialog
        :open="updater.updateDialogOpen.value"
        :status="updater.updateStatus.value"
        :update="updater.availableUpdate.value"
        :error="updater.updateError.value"
        :error-phase="updater.updateErrorPhase.value"
        :downloaded-bytes="updater.updateDownloadedBytes.value"
        :total-bytes="updater.updateTotalBytes.value"
        @dismiss="updater.dismissUpdateDialog"
        @install="updater.installAvailableUpdate"
        @relaunch="updater.relaunchForUpdate"
      />

      <section
        class="main-content"
        :class="{ 'main-content-side': isSideLayout }"
      >
        <CategoryRail
          ref="categoryRailElement"
          :categories="store.categories"
          :selected-category-id="store.selectedCategoryId"
          :editing-category-id="editingCategoryId"
          :history-count="store.clipTotalCount"
          :category-counts="categoryItemCounts"
          :orientation="isSideLayout ? 'vertical' : 'horizontal'"
          @select="store.selectCategory"
          @create="createCategory"
          @edit="editCategory"
          @rename="renameCategory"
          @recolor="updateCategoryColor"
          @finish-editing="finishEditingCategory"
          @delete="deleteCategory"
          @reorder="reorderCategories"
        />

        <section class="clip-area">
          <div v-if="store.error" class="mx-4 mt-4 flex items-center gap-2 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700">
            <AlertCircle class="size-4" />
            <span class="min-w-0 flex-1 truncate">{{ store.error }}</span>
          </div>

          <div
            ref="clipListElement"
            class="clip-list-scroll subtle-scrollbar min-h-0 flex-1 overflow-y-auto p-4"
            :class="{
              'subtle-scrollbar-active': isClipListScrolling,
              'clip-list-scroll-previewing': quickPreviewItem,
            }"
            @scroll="handleClipListScroll"
            @pointerleave="clearQuickPreviewHover"
          >
            <div
              v-if="store.isLoading"
              class="clip-card-grid"
            >
              <div v-for="index in 9" :key="index" class="h-40 animate-pulse rounded-lg border border-slate-200 bg-white" />
            </div>

            <div
              v-else-if="store.visibleItems.length"
              class="clip-card-grid"
            >
              <ClipCard
                v-for="(item, index) in store.visibleItems"
                :key="`${item.collection}-${item.id}`"
                :item="item"
                :index="index"
                :data-item-key="contextItemKey(item)"
                :data-item-id="item.id"
                :selected="store.selectedIndex === index"
                :category-tags="itemCategoryTags(item)"
                :editing-name="editingClipKey === contextItemKey(item) ? editingClipName : null"
                :reorder-enabled="canReorderVisibleItems && item.collection === 'category'"
                :style="itemDragStyle(item)"
                :class="{
                  'clip-card-dragging': draggingItemKey === contextItemKey(item),
                  'clip-card-drop-before': itemDropTargetKey === contextItemKey(item) && itemDropSide === 'before',
                  'clip-card-drop-after': itemDropTargetKey === contextItemKey(item) && itemDropSide === 'after',
                }"
                @select="selectClipCard"
                @apply="store.applyItem"
                @expand="openClipViewer"
                @open-context-menu="openClipContextMenu"
                @update-editing-name="updateEditingClipName"
                @commit-rename="commitEditingClipName"
                @cancel-rename="cancelEditingClipName"
                @reorder-pointer-down="startItemDrag"
                @pointerenter="hoverPreviewItem(item)"
                @pointerleave="clearHoveredPreviewItem(item)"
              />
              <div
                v-if="store.selectedCategoryId === 'history' && store.isLoadingMoreClips"
                class="clip-grid-full h-24 animate-pulse rounded-lg border border-slate-200 bg-white"
              />
            </div>

            <div v-else class="flex h-full min-h-[360px] flex-col items-center justify-center rounded-lg border border-dashed border-slate-300 bg-white/70 text-center">
              <Inbox class="size-10 text-slate-300" />
              <h2 class="mt-3 text-base font-semibold text-slate-900">{{ t("empty.title") }}</h2>
              <p class="mt-1 max-w-sm text-sm text-slate-500">
                {{ t("empty.description") }}
              </p>
            </div>
          </div>

          <div
            v-if="quickPreviewItem"
            class="quick-preview-overlay"
            :class="{ 'quick-preview-overlay-locked': isQuickPreviewLocked }"
            role="dialog"
            :aria-label="quickPreviewAriaLabel"
            @pointerdown.stop="lockQuickPreview"
            @click.stop="lockQuickPreview"
            @contextmenu.stop
          >
            <div class="quick-preview-meta">
              <span class="quick-preview-type">{{ typeLabel(quickPreviewItem.clipType) }}</span>
              <span v-if="quickPreviewTitle" class="quick-preview-title">{{ quickPreviewTitle }}</span>
              <span class="quick-preview-spacer" />
              <span>{{ formatTime(quickPreviewTime) }}</span>
              <span v-if="quickPreviewSize">{{ quickPreviewSize }}</span>
              <button
                type="button"
                class="quick-preview-action-button"
                :disabled="!quickPreviewSelectedText.trim()"
                tabindex="-1"
                :aria-label="t('common.paste')"
                :data-tooltip="t('common.paste')"
                @pointerdown.stop
                @click.stop="pasteQuickPreviewSelection"
              >
                <CornerDownLeft class="size-3.5" />
                <span>{{ t("common.paste") }}</span>
              </button>
              <button
                type="button"
                class="quick-preview-icon-button"
                tabindex="-1"
                :aria-label="t('common.copy')"
                :data-tooltip="t('common.copy')"
                @pointerdown.stop
                @click.stop="copyQuickPreviewItem"
              >
                <ClipboardCopy class="size-3.5" />
              </button>
              <button
                type="button"
                class="quick-preview-icon-button"
                tabindex="-1"
                :aria-label="t('common.close')"
                :data-tooltip="t('common.close')"
                @pointerdown.stop
                @click.stop="closeQuickPreview"
              >
                <X class="size-3.5" />
              </button>
            </div>

            <div v-if="quickPreviewItem.clipType === 'image'" class="quick-preview-image">
              <img :src="quickPreviewImageSrc" :alt="t('common.imagePreviewAlt')" />
            </div>

            <div v-else-if="quickPreviewItem.clipType === 'color'" class="quick-preview-color">
              <span class="quick-preview-color-swatch" :style="{ backgroundColor: quickPreviewColorValue }" />
              <code>{{ quickPreviewContent }}</code>
            </div>

            <div v-else class="quick-preview-text">{{ quickPreviewContent }}</div>
          </div>
        </section>
      </section>
    </section>

    <div
      v-if="contextMenu"
      ref="contextMenuElement"
      class="clip-context-menu"
      :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }"
      role="menu"
      @click.stop
      @contextmenu.prevent.stop
      @mouseleave="pendingDeleteContextKey = null"
    >
      <button type="button" class="context-menu-item context-menu-item-strong" tabindex="-1" role="menuitem" @click="pasteContextItem">
        <CornerDownLeft class="size-4" />
        <span>{{ t("common.paste") }}</span>
      </button>
      <button type="button" class="context-menu-item" tabindex="-1" role="menuitem" @click="copyContextItem">
        <ClipboardCopy class="size-4" />
        <span>{{ t("common.copy") }}</span>
      </button>
      <div class="context-menu-separator" />
      <button type="button" class="context-menu-item" tabindex="-1" role="menuitem" @click="renameContextItem">
        <Pencil class="size-4" />
        <span>{{ t("common.rename") }}</span>
      </button>
      <div
        ref="moveSubmenuBranchElement"
        class="context-menu-branch"
        :class="{ 'context-menu-branch-left': submenuAlignLeft }"
        @mouseenter="openMoveSubmenu"
        @mouseleave="scheduleCloseMoveSubmenu"
      >
        <button type="button" class="context-menu-item" tabindex="-1" role="menuitem" @click.stop="openMoveSubmenu">
          <FolderInput class="size-4" />
          <span>{{ t("context.moveTo") }}</span>
          <ChevronRight class="ml-auto size-4 text-slate-400" />
        </button>
        <div
          v-if="showMoveSubmenu"
          ref="moveSubmenuElement"
          class="clip-context-submenu"
          :class="{ 'clip-context-submenu-left': submenuAlignLeft }"
          :style="{ top: `${submenuOffsetTop}px` }"
          @mouseenter="openMoveSubmenu"
          @mouseleave="scheduleCloseMoveSubmenu"
        >
          <button
            v-for="category in store.categories"
            :key="category.id"
            type="button"
            class="context-menu-item"
            tabindex="-1"
            role="menuitem"
            @click="addContextItemToCategory(category.id)"
          >
            <span class="size-2 rounded-full" :style="{ backgroundColor: category.color }" />
            <span class="min-w-0 flex-1 truncate">{{ categoryDisplayName(category.name) }}</span>
          </button>
          <div v-if="store.categories.length" class="context-menu-separator" />
          <button type="button" class="context-menu-item" tabindex="-1" role="menuitem" @click="createCategoryForContextItem">
            <Plus class="size-4" />
            <span>{{ t("context.createCategory") }}</span>
          </button>
        </div>
      </div>
      <div class="context-menu-separator" />
      <button
        type="button"
        class="context-menu-item context-menu-item-danger"
        :class="{ 'context-menu-item-confirm': pendingDeleteContextKey === contextItemKey(contextMenu.item) }"
        tabindex="-1"
        role="menuitem"
        @click="deleteContextItem"
        @mouseleave="pendingDeleteContextKey = null"
      >
        <Trash2 class="size-4" />
        <span>{{ contextDeleteLabel(contextMenu.item) }}</span>
      </button>
    </div>
  </main>
</template>
