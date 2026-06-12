import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { ipasteApi } from "../lib/ipasteApi";
import type {
  AppendCopyChangedEvent,
  CapturedEvent,
  Category,
  CategoryItem,
  ClipItem,
  ClipUpdatedEvent,
  ClipViewItem,
  CloudSettings,
  ListeningChangedEvent,
  OcrMode,
  PanelLayout,
  PanelOpenBehavior,
  SettingsChangedEvent,
} from "../types";

const CATEGORY_COLORS = ["#0D9488", "#2563EB", "#7C3AED", "#D97706", "#DC2626", "#475569"];
const DEFAULT_RETENTION_DAYS = 30;
const DEFAULT_APPEND_COPY_TIMEOUT_MINUTES = 1;
const APPEND_COPY_TIMEOUT_OPTIONS = [1, 3, 5, 10];
const DEFAULT_PANEL_LAYOUT: PanelLayout = "top";
const DEFAULT_OCR_MODE: OcrMode = "fast";
const CLIP_PAGE_SIZE = 20;
const isTauri = "__TAURI_INTERNALS__" in window;

export const useIpasteStore = defineStore("ipaste", () => {
  const clips = ref<ClipItem[]>([]);
  const categories = ref<Category[]>([]);
  const categoryItems = ref<CategoryItem[]>([]);
  const selectedCategoryId = ref<string>("history");
  const selectedIndex = ref(0);
  const search = ref("");
  const shortcut = ref("CommandOrControl+Shift+V");
  const isListening = ref(true);
  const isAppendCopyEnabled = ref(false);
  const isLoading = ref(false);
  const isLoadingMoreClips = ref(false);
  const hasMoreClips = ref(false);
  const clipTotalCount = ref(0);
  const visibleHistoryTotalCount = ref(0);
  const error = ref<string | null>(null);
  const retentionDays = ref(DEFAULT_RETENTION_DAYS);
  const appendCopyTimeoutMinutes = ref(DEFAULT_APPEND_COPY_TIMEOUT_MINUTES);
  const panelOpenBehavior = ref<PanelOpenBehavior>("history");
  const panelLayout = ref<PanelLayout>(DEFAULT_PANEL_LAYOUT);
  const ocrMode = ref<OcrMode>(DEFAULT_OCR_MODE);
  const cloud = ref<CloudSettings>({
    apiAddress: "",
    apiKey: "",
    enabled: false,
    lastConnectedAt: null,
  });
  let backgroundSyncTimer: number | null = null;
  let clipRequestId = 0;

  const activeCategory = computed(() =>
    categories.value.find((category) => category.id === selectedCategoryId.value),
  );

  const visibleItems = computed<ClipViewItem[]>(() => {
    const query = search.value.trim().toLowerCase();
    const source =
      selectedCategoryId.value === "history"
        ? clips.value.map((clip) => ({ ...clip, collection: "history" as const }))
        : categoryItems.value
            .filter((item) => item.categoryId === selectedCategoryId.value)
            .map((item) => ({ ...item, collection: "category" as const }));

    if (!query) return source;

    return source.filter((item) =>
      [
        item.displayName ?? "",
        item.previewText,
        item.clipType,
        item.clipType === "image" ? "图片 image" : item.text,
      ].some((value) => value.toLowerCase().includes(query)),
    );
  });

  const selectedItem = computed(() => visibleItems.value[selectedIndex.value]);

  async function load() {
    isLoading.value = true;
    error.value = null;

    try {
      const snapshot = await ipasteApi.snapshot();
      clips.value = snapshot.clips;
      hasMoreClips.value = snapshot.hasMoreClips;
      clipTotalCount.value = snapshot.clipTotalCount;
      visibleHistoryTotalCount.value = snapshot.clipTotalCount;
      categories.value = snapshot.categories;
      categoryItems.value = snapshot.categoryItems;
      shortcut.value = snapshot.shortcut;
      isListening.value = snapshot.isListening;
      isAppendCopyEnabled.value = snapshot.isAppendCopyEnabled;
      retentionDays.value = snapshot.settings.retentionDays;
      appendCopyTimeoutMinutes.value = cleanAppendCopyTimeoutMinutes(snapshot.settings.appendCopyTimeoutMinutes);
      panelOpenBehavior.value = snapshot.settings.panelOpenBehavior;
      panelLayout.value = cleanPanelLayout(snapshot.settings.panelLayout);
      ocrMode.value = cleanOcrMode(snapshot.settings.ocrMode);
      cloud.value = snapshot.settings.cloud;

      if (!categories.value.some((category) => category.id === selectedCategoryId.value)) {
        selectedCategoryId.value = "history";
      }
      clampSelection();
    } catch (unknownError) {
      error.value = String(unknownError);
    } finally {
      isLoading.value = false;
    }
  }

  async function bindEvents() {
    if (!isTauri) return;

    await listen<CapturedEvent>("ipaste://clipboard-captured", (event) => {
      upsertClip(event.payload.clip, event.payload.clipTotalCount, event.payload.wasInserted);
    });

    await listen<ListeningChangedEvent>("ipaste://listening-changed", (event) => {
      isListening.value = event.payload.isListening;
    });

    await listen<AppendCopyChangedEvent>("ipaste://append-copy-changed", (event) => {
      isAppendCopyEnabled.value = event.payload.isEnabled;
    });

    await listen<ClipUpdatedEvent>("ipaste://clip-updated", (event) => {
      patchItem(event.payload.collection, event.payload.item);
      if (event.payload.collection === "category") {
        syncCloudInBackground();
      }
    });

    await listen<SettingsChangedEvent>("ipaste://settings-changed", (event) => {
      applySettings(event.payload.settings);
    });

    await listen<{ visible: boolean }>("ipaste://panel-visibility-changed", (event) => {
      if (event.payload.visible) {
        activatePanelDefault();
      }
    });
  }

  async function createCategory(name: string, options: { select?: boolean } = {}) {
    const color = CATEGORY_COLORS[categories.value.length % CATEGORY_COLORS.length];
    const category = await ipasteApi.createCategory(name, color);
    categories.value = [...categories.value, category].sort(compareSortOrder);
    syncCloudInBackground();
    if (options.select ?? true) {
      selectedCategoryId.value = category.id;
      selectedIndex.value = 0;
    }
    return category;
  }

  async function loadMoreClips() {
    if (selectedCategoryId.value !== "history" || isLoadingMoreClips.value || !hasMoreClips.value) return;

    isLoadingMoreClips.value = true;
    try {
      const page = await ipasteApi.listClips(clips.value.length, CLIP_PAGE_SIZE, search.value);
      const existingIds = new Set(clips.value.map((clip) => clip.id));
      clips.value = [
        ...clips.value,
        ...page.clips.filter((clip) => !existingIds.has(clip.id)),
      ];
      hasMoreClips.value = page.hasMore;
      visibleHistoryTotalCount.value = page.totalCount;
      clipTotalCount.value = page.allCount;
      clampSelection();
    } catch (unknownError) {
      error.value = String(unknownError);
    } finally {
      isLoadingMoreClips.value = false;
    }
  }

  async function reloadClips() {
    const requestId = ++clipRequestId;

    try {
      const page = await ipasteApi.listClips(0, CLIP_PAGE_SIZE, search.value);
      if (requestId !== clipRequestId) return;

      clips.value = page.clips;
      hasMoreClips.value = page.hasMore;
      visibleHistoryTotalCount.value = page.totalCount;
      clipTotalCount.value = page.allCount;
      selectedIndex.value = 0;
    } catch (unknownError) {
      if (requestId === clipRequestId) {
        error.value = String(unknownError);
      }
    }
  }

  async function createCategoryWithClip(name: string, clipId: string, options: { select?: boolean } = {}) {
    const color = CATEGORY_COLORS[categories.value.length % CATEGORY_COLORS.length];
    const { category, item } = await ipasteApi.createCategoryWithClip(name, color, clipId);
    categories.value = [...categories.value, category].sort(compareSortOrder);
    categoryItems.value = [...categoryItems.value, item].sort(compareCategoryItemOrder);
    clips.value = clips.value.map((clip) =>
      clip.id === clipId ? { ...clip, favoriteCount: clip.favoriteCount + 1 } : clip,
    );
    syncCloudInBackground();
    if (options.select ?? true) {
      selectedCategoryId.value = category.id;
      selectedIndex.value = 0;
    }
    return { category, item };
  }

  async function renameCategory(category: Category, name: string) {
    const next = await ipasteApi.updateCategory(category.id, name, category.color);
    categories.value = categories.value.map((item) => (item.id === next.id ? next : item));
    syncCloudInBackground();
  }

  async function updateCategoryColor(category: Category, color: string) {
    const next = await ipasteApi.updateCategory(category.id, category.name, color);
    categories.value = categories.value.map((item) => (item.id === next.id ? next : item));
    syncCloudInBackground();
  }

  async function deleteCategory(id: string) {
    await ipasteApi.deleteCategory(id);
    categories.value = categories.value.filter((category) => category.id !== id);
    categoryItems.value = categoryItems.value.filter((item) => item.categoryId !== id);
    selectedCategoryId.value = "history";
    selectedIndex.value = 0;
    syncCloudInBackground();
  }

  async function addToCategory(clipId: string, categoryId: string) {
    const item = await ipasteApi.addClipToCategory(clipId, categoryId);
    const existing = categoryItems.value.some((categoryItem) => categoryItem.id === item.id);
    if (!existing) {
      categoryItems.value = [...categoryItems.value, item].sort(compareCategoryItemOrder);
      clips.value = clips.value.map((clip) =>
        clip.id === clipId ? { ...clip, favoriteCount: clip.favoriteCount + 1 } : clip,
      );
      syncCloudInBackground();
    }
  }

  async function removeCategoryItem(id: string) {
    await ipasteApi.removeCategoryItem(id);
    categoryItems.value = categoryItems.value.filter((item) => item.id !== id);
    clampSelection();
    syncCloudInBackground();
  }

  async function deleteClip(id: string) {
    await ipasteApi.deleteClip(id);
    const hadClip = clips.value.some((clip) => clip.id === id);
    clips.value = clips.value.filter((clip) => clip.id !== id);
    if (hadClip) {
      clipTotalCount.value = Math.max(0, clipTotalCount.value - 1);
      visibleHistoryTotalCount.value = Math.max(0, visibleHistoryTotalCount.value - 1);
    }
    clampSelection();
  }

  async function renameClip(item: ClipViewItem, displayName: string | null) {
    const next = await ipasteApi.renameClip(item.id, item.collection, displayName);
    patchItem(item.collection, next);
    if (item.collection === "category") {
      syncCloudInBackground();
    }
  }

  async function reorderCategories(categoryIds: string[]) {
    if (categoryIds.length !== categories.value.length) return;

    const previous = categories.value;
    categories.value = orderCategoriesByIds(previous, categoryIds);

    try {
      categories.value = await ipasteApi.reorderCategories(categoryIds);
      syncCloudInBackground();
    } catch (unknownError) {
      categories.value = previous;
      error.value = String(unknownError);
      throw unknownError;
    }
  }

  async function reorderCategoryItems(categoryId: string, itemIds: string[]) {
    const targetItems = categoryItems.value.filter((item) => item.categoryId === categoryId);
    if (itemIds.length !== targetItems.length) return;

    const previous = categoryItems.value;
    const selectedItemId = selectedItem.value?.collection === "category" ? selectedItem.value.id : null;
    categoryItems.value = orderCategoryItemsByIds(previous, categoryId, itemIds);
    restoreCategorySelection(selectedItemId);

    try {
      categoryItems.value = await ipasteApi.reorderCategoryItems(categoryId, itemIds);
      restoreCategorySelection(selectedItemId);
      syncCloudInBackground();
    } catch (unknownError) {
      categoryItems.value = previous;
      restoreCategorySelection(selectedItemId);
      error.value = String(unknownError);
      throw unknownError;
    }
  }

  async function updateClipContent(item: ClipViewItem, text: string) {
    const next = await ipasteApi.updateClipContent(item.id, item.collection, text);
    patchItem(item.collection, next);
    if (item.collection === "category") {
      syncCloudInBackground();
    }
    return next;
  }

  async function applySelected() {
    if (!selectedItem.value) return;
    await ipasteApi.applyClip(
      originalClipId(selectedItem.value),
      selectedItem.value.clipType,
      selectedItem.value.text,
    );
  }

  async function applyItem(item: ClipViewItem) {
    await ipasteApi.applyClip(originalClipId(item), item.clipType, item.text);
  }

  async function copyItem(item: ClipViewItem) {
    await ipasteApi.copyClip(item.clipType, item.text);
  }

  async function setAppendCopyEnabled(enabled: boolean) {
    try {
      isAppendCopyEnabled.value = await ipasteApi.setAppendCopyEnabled(enabled);
    } catch (unknownError) {
      error.value = String(unknownError);
      throw unknownError;
    }
  }

  async function toggleAppendCopy() {
    await setAppendCopyEnabled(!isAppendCopyEnabled.value);
  }

  async function hidePanel() {
    await ipasteApi.hidePanel();
  }

  async function showSettings() {
    await ipasteApi.showSettings();
  }

  async function updateRetentionDays(days: number) {
    const settings = await ipasteApi.updateSettings(days);
    applySettings(settings);
    await load();
  }

  async function updateAppendCopyTimeout(minutes: number) {
    const nextMinutes = cleanAppendCopyTimeoutMinutes(minutes);
    appendCopyTimeoutMinutes.value = nextMinutes;

    try {
      const settings = await ipasteApi.updateAppendCopyTimeout(nextMinutes);
      applySettings(settings);
    } catch (unknownError) {
      const message = String(unknownError);
      if (message.includes("update_append_copy_timeout") && message.includes("not found")) {
        return;
      }

      error.value = message;
      throw unknownError;
    }
  }

  async function updateShortcut(value: string) {
    const settings = await ipasteApi.updateShortcut(value);
    applySettings(settings);
  }

  async function updatePanelOpenBehavior(behavior: PanelOpenBehavior) {
    const settings = await ipasteApi.updatePanelOpenBehavior(behavior);
    applySettings(settings);
  }

  async function updatePanelLayout(layout: PanelLayout) {
    const nextLayout = cleanPanelLayout(layout);
    panelLayout.value = nextLayout;

    try {
      const settings = await ipasteApi.updatePanelLayout(nextLayout);
      applySettings(settings);
    } catch (unknownError) {
      const message = String(unknownError);
      if (message.includes("update_panel_layout") && message.includes("not found")) {
        return;
      }

      error.value = message;
      throw unknownError;
    }
  }

  async function updateOcrMode(mode: OcrMode) {
    const nextMode = cleanOcrMode(mode);
    ocrMode.value = nextMode;

    try {
      const settings = await ipasteApi.updateOcrMode(nextMode);
      applySettings(settings);
    } catch (unknownError) {
      const message = String(unknownError);
      if (message.includes("update_ocr_mode") && message.includes("not found")) {
        return;
      }

      error.value = message;
      throw unknownError;
    }
  }

  async function saveCloudSettings(apiAddress: string, apiKey: string) {
    const settings = await ipasteApi.updateCloudSettings(apiAddress, apiKey);
    applySettings(settings);
    await syncCloudNow();
  }

  async function disableCloudSync() {
    const settings = await ipasteApi.disableCloudSync();
    applySettings(settings);
  }

  async function testCloudSettings(apiAddress: string, apiKey: string) {
    return ipasteApi.testCloudSettings(apiAddress, apiKey);
  }

  async function syncCloudNow() {
    if (!cloud.value.enabled) return;

    try {
      clearBackgroundSyncTimer();
      await applyCloudSnapshot();
    } catch (unknownError) {
      error.value = String(unknownError);
      throw unknownError;
    }
  }

  function syncCloudInBackground() {
    if (!cloud.value.enabled) return;
    clearBackgroundSyncTimer();
    backgroundSyncTimer = window.setTimeout(() => {
      backgroundSyncTimer = null;
      void ipasteApi.syncCloudInBackground().catch((unknownError) => {
        error.value = String(unknownError);
      });
    }, 600);
  }

  async function applyCloudSnapshot() {
    const snapshot = await ipasteApi.syncCloudNow();
    clips.value = snapshot.clips;
    hasMoreClips.value = snapshot.hasMoreClips;
    clipTotalCount.value = snapshot.clipTotalCount;
    visibleHistoryTotalCount.value = snapshot.clipTotalCount;
    categories.value = snapshot.categories;
    categoryItems.value = snapshot.categoryItems;
    shortcut.value = snapshot.shortcut;
    isListening.value = snapshot.isListening;
    isAppendCopyEnabled.value = snapshot.isAppendCopyEnabled;
    retentionDays.value = snapshot.settings.retentionDays;
    appendCopyTimeoutMinutes.value = cleanAppendCopyTimeoutMinutes(snapshot.settings.appendCopyTimeoutMinutes);
    panelOpenBehavior.value = snapshot.settings.panelOpenBehavior;
    panelLayout.value = cleanPanelLayout(snapshot.settings.panelLayout);
    ocrMode.value = cleanOcrMode(snapshot.settings.ocrMode);
    cloud.value = snapshot.settings.cloud;
    clampSelection();
  }

  function clearBackgroundSyncTimer() {
    if (backgroundSyncTimer === null) return;
    window.clearTimeout(backgroundSyncTimer);
    backgroundSyncTimer = null;
  }

  function applySettings(settings: {
    shortcut: string;
    retentionDays: number;
    appendCopyTimeoutMinutes?: number;
    panelOpenBehavior: PanelOpenBehavior;
    panelLayout?: PanelLayout;
    ocrMode?: OcrMode;
    cloud: CloudSettings;
  }) {
    shortcut.value = settings.shortcut;
    retentionDays.value = settings.retentionDays;
    appendCopyTimeoutMinutes.value = cleanAppendCopyTimeoutMinutes(settings.appendCopyTimeoutMinutes);
    panelOpenBehavior.value = settings.panelOpenBehavior;
    panelLayout.value = cleanPanelLayout(settings.panelLayout);
    ocrMode.value = cleanOcrMode(settings.ocrMode);
    cloud.value = settings.cloud;
  }

  function cleanAppendCopyTimeoutMinutes(minutes: unknown) {
    const normalized = Number(minutes);
    return APPEND_COPY_TIMEOUT_OPTIONS.includes(normalized)
      ? normalized
      : DEFAULT_APPEND_COPY_TIMEOUT_MINUTES;
  }

  function cleanPanelLayout(layout: unknown): PanelLayout {
    return layout === "side" ? "side" : DEFAULT_PANEL_LAYOUT;
  }

  function cleanOcrMode(mode: unknown): OcrMode {
    return mode === "best" ? "best" : DEFAULT_OCR_MODE;
  }

  function selectCategory(id: string) {
    selectedCategoryId.value = id;
    selectedIndex.value = 0;
    if (id === "history") {
      void reloadClips();
    }
  }

  function activatePanelDefault() {
    if (panelOpenBehavior.value === "history" || !categories.value.some((category) => category.id === selectedCategoryId.value)) {
      selectCategory("history");
      return;
    }

    selectedIndex.value = 0;
  }

  function moveSelection(delta: number) {
    if (!visibleItems.value.length) return;
    const next = selectedIndex.value + delta;
    selectedIndex.value = Math.min(Math.max(next, 0), visibleItems.value.length - 1);
  }

  function setSelectedIndex(index: number) {
    selectedIndex.value = index;
  }

  function clampSelection() {
    if (!visibleItems.value.length) {
      selectedIndex.value = 0;
      return;
    }

    selectedIndex.value = Math.min(selectedIndex.value, visibleItems.value.length - 1);
  }

  function upsertClip(clip: ClipItem, totalCount?: number, wasInserted = false) {
    const hadClip = clips.value.some((item) => item.id === clip.id);
    const hasSearch = Boolean(search.value.trim());
    const matchesCurrentSearch = clipMatchesSearch(clip, search.value);

    if (!hasSearch || matchesCurrentSearch) {
      clips.value = [clip, ...clips.value.filter((item) => item.id !== clip.id)].slice(0, 120);
    }

    if (typeof totalCount === "number") {
      clipTotalCount.value = totalCount;
      if (!hasSearch) {
        visibleHistoryTotalCount.value = totalCount;
      } else if (wasInserted && clipMatchesSearch(clip, search.value)) {
        visibleHistoryTotalCount.value += 1;
      }
    } else if (!hadClip && !hasMoreClips.value) {
      clipTotalCount.value += 1;
      visibleHistoryTotalCount.value += 1;
    }
    if (!hasSearch) {
      hasMoreClips.value = hasMoreClips.value || clips.value.length >= CLIP_PAGE_SIZE;
    }
    if (selectedCategoryId.value === "history") {
      selectedIndex.value = 0;
    }
  }

  function patchItem(collection: "history" | "category", item: ClipItem | CategoryItem) {
    if (collection === "history") {
      clips.value = clips.value.map((clip) => (clip.id === item.id ? (item as ClipItem) : clip));
      return;
    }

    categoryItems.value = categoryItems.value.map((categoryItem) =>
      categoryItem.id === item.id ? (item as CategoryItem) : categoryItem,
    );
  }

  function originalClipId(item: ClipViewItem) {
    return item.collection === "history" ? item.id : item.clipSnapshotId;
  }

  function clipMatchesSearch(clip: ClipItem, value: string) {
    const query = value.trim().toLowerCase();
    if (!query) return true;

    return [
      clip.displayName ?? "",
      clip.previewText,
      clip.clipType,
      clip.clipType === "image" ? "图片 image" : clip.text,
    ].some((text) => text.toLowerCase().includes(query));
  }

  function orderCategoriesByIds(items: Category[], ids: string[]) {
    const byId = new Map(items.map((item) => [item.id, item]));
    return ids
      .map((id, index) => {
        const item = byId.get(id);
        return item ? { ...item, sortOrder: index } : null;
      })
      .filter((item): item is Category => Boolean(item));
  }

  function orderCategoryItemsByIds(items: CategoryItem[], categoryId: string, ids: string[]) {
    const byId = new Map(items.filter((item) => item.categoryId === categoryId).map((item) => [item.id, item]));
    const reordered = ids
      .map((id, index) => {
        const item = byId.get(id);
        return item ? { ...item, sortOrder: index } : null;
      })
      .filter((item): item is CategoryItem => Boolean(item));
    return [
      ...items.filter((item) => item.categoryId !== categoryId),
      ...reordered,
    ].sort(compareCategoryItemOrder);
  }

  function compareSortOrder(left: Category, right: Category) {
    return left.sortOrder - right.sortOrder || left.createdAt.localeCompare(right.createdAt);
  }

  function compareCategoryItemOrder(left: CategoryItem, right: CategoryItem) {
    if (left.categoryId !== right.categoryId) return left.categoryId.localeCompare(right.categoryId);
    if (left.isPinned !== right.isPinned) return left.isPinned ? -1 : 1;
    return left.sortOrder - right.sortOrder || right.createdAt.localeCompare(left.createdAt);
  }

  function restoreCategorySelection(itemId: string | null) {
    if (!itemId) {
      clampSelection();
      return;
    }

    const index = visibleItems.value.findIndex((item) => item.collection === "category" && item.id === itemId);
    if (index >= 0) {
      selectedIndex.value = index;
      return;
    }

    clampSelection();
  }

  return {
    clips,
    categories,
    categoryItems,
    selectedCategoryId,
    selectedIndex,
    search,
    shortcut,
    isListening,
    isAppendCopyEnabled,
    isLoading,
    isLoadingMoreClips,
    hasMoreClips,
    clipTotalCount,
    visibleHistoryTotalCount,
    error,
    retentionDays,
    appendCopyTimeoutMinutes,
    panelOpenBehavior,
    panelLayout,
    ocrMode,
    cloud,
    activeCategory,
    visibleItems,
    selectedItem,
    load,
    reloadClips,
    loadMoreClips,
    bindEvents,
    createCategory,
    createCategoryWithClip,
    renameCategory,
    updateCategoryColor,
    deleteCategory,
    addToCategory,
    reorderCategories,
    reorderCategoryItems,
    removeCategoryItem,
    deleteClip,
    renameClip,
    updateClipContent,
    applySelected,
    applyItem,
    copyItem,
    setAppendCopyEnabled,
    toggleAppendCopy,
    hidePanel,
    showSettings,
    updateRetentionDays,
    updateAppendCopyTimeout,
    updateShortcut,
    updatePanelOpenBehavior,
    updatePanelLayout,
    updateOcrMode,
    saveCloudSettings,
    disableCloudSync,
    testCloudSettings,
    syncCloudNow,
    syncCloudInBackground,
    selectCategory,
    activatePanelDefault,
    moveSelection,
    setSelectedIndex,
    clampSelection,
  };
});
