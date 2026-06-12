import { invoke } from "@tauri-apps/api/core";
import appPackage from "../../package.json";
import type {
  AppInfo,
  AppSettings,
  AppSnapshot,
  Category,
  CategoryItem,
  CategoryWithItem,
  ClipItem,
  ClipPage,
  ClipViewerPayload,
  ClipViewItem,
  ImageOcrResult,
  OcrMode,
  OcrInstallStatus,
} from "../types";

const isTauri = "__TAURI_INTERNALS__" in window;
const fallbackAppInfo: AppInfo = {
  version: appPackage.version,
};
const fallbackOcrInstallStatus: OcrInstallStatus = {
  installed: false,
  engineId: "tesseract",
  engineVersion: null,
  mode: "fast",
  platform: "windows-x64",
  manifestUrl: "https://github.com/iPaste-app/iPaste/releases/download/ipaste-ocr-windows-v1/ipaste-ocr-windows-x64-fast.json",
  installDir: "",
  downloadedBytes: 0,
  totalBytes: 37_557_099,
  missingFiles: [],
};

const mockCategories: Category[] = [
  {
    id: "dev",
    name: "开发片段",
    color: "#2563EB",
    sortOrder: 0,
    createdAt: new Date(Date.now() - 42_400_000).toISOString(),
    updatedAt: new Date(Date.now() - 42_400_000).toISOString(),
  },
];

const mockClips: ClipItem[] = [
  {
    id: "clip-image",
    clipType: "image",
    contentHash: "mock-image",
    displayName: null,
    previewText: "图片 240 x 160",
    text: "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='240' height='160' viewBox='0 0 240 160'%3E%3Crect width='240' height='160' rx='18' fill='%23dbeafe'/%3E%3Ccircle cx='76' cy='70' r='28' fill='%230d9488'/%3E%3Cpath d='M40 126l48-42 34 30 28-24 50 36z' fill='%232563eb' opacity='.75'/%3E%3C/svg%3E",
    sourceApp: "预览",
    lastCapturedAt: new Date(Date.now() - 60_000).toISOString(),
    favoriteCount: 0,
    isPinned: false,
  },
  {
    id: "clip-link",
    clipType: "link",
    contentHash: "mock-link",
    displayName: "Tauri 剪贴板文档",
    previewText: "https://tauri.app/plugin/clipboard/",
    text: "https://tauri.app/plugin/clipboard/",
    sourceApp: "Safari",
    lastCapturedAt: new Date(Date.now() - 120_000).toISOString(),
    favoriteCount: 1,
    isPinned: false,
  },
  {
    id: "clip-color",
    clipType: "color",
    contentHash: "mock-color",
    displayName: null,
    previewText: "#0D9488",
    text: "#0D9488",
    sourceApp: "Figma",
    lastCapturedAt: new Date(Date.now() - 500_000).toISOString(),
    favoriteCount: 0,
    isPinned: false,
  },
  {
    id: "clip-text",
    clipType: "text",
    contentHash: "mock-text",
    displayName: null,
    previewText: "使用 Tauri 命令处理原生剪贴板捕捉，让 Vue 状态只关注界面交互。",
    text: "使用 Tauri 命令处理原生剪贴板捕捉，让 Vue 状态只关注界面交互。",
    sourceApp: "备忘录",
    lastCapturedAt: new Date(Date.now() - 1_100_000).toISOString(),
    favoriteCount: 2,
    isPinned: false,
  },
];

const mockCategoryItems: CategoryItem[] = [
  {
    id: "saved-text",
    categoryId: "dev",
    clipSnapshotId: "clip-text",
    clipType: "text",
    contentHash: "mock-text",
    displayName: "Tauri 状态说明",
    previewText: "使用 Tauri 命令处理原生剪贴板捕捉，让 Vue 状态只关注界面交互。",
    text: "使用 Tauri 命令处理原生剪贴板捕捉，让 Vue 状态只关注界面交互。",
    sortOrder: 0,
    createdAt: new Date(Date.now() - 90_000).toISOString(),
    updatedAt: new Date(Date.now() - 90_000).toISOString(),
    syncState: "local",
    isPinned: false,
  },
];

const mockSnapshot: AppSnapshot = {
  clips: mockClips,
  hasMoreClips: false,
  clipTotalCount: mockClips.length,
  categories: mockCategories,
  categoryItems: mockCategoryItems,
  shortcut: "CommandOrControl+Shift+V",
  isListening: true,
  isAppendCopyEnabled: false,
  settings: {
    shortcut: "CommandOrControl+Shift+V",
    retentionDays: 30,
    appendCopyTimeoutMinutes: 1,
    panelOpenBehavior: "history",
    panelLayout: "top",
    ocrMode: "fast",
    cloud: {
      apiAddress: "",
      apiKey: "",
      enabled: false,
      lastConnectedAt: null,
    },
  },
};

async function call<T>(command: string, args?: Record<string, unknown>, fallback?: T) {
  if (isTauri) return invoke<T>(command, args);
  if (fallback !== undefined) return structuredClone(fallback);
  return undefined as T;
}

export const ipasteApi = {
  snapshot() {
    return call<AppSnapshot>("get_snapshot", undefined, mockSnapshot);
  },
  listClips(offset = 0, limit = 20, search = "") {
    const query = search.trim().toLowerCase();
    const source = query
      ? mockClips.filter((item) =>
          [
            item.displayName ?? "",
            item.previewText,
            item.clipType,
            item.clipType === "image" ? "图片 image" : item.text,
          ].some((value) => value.toLowerCase().includes(query)),
        )
      : mockClips;
    return call<ClipPage>("list_clips", { offset, limit, search }, {
      clips: source.slice(offset, offset + limit),
      hasMore: offset + limit < source.length,
      totalCount: source.length,
      allCount: mockClips.length,
    });
  },
  listCategories() {
    return call<Category[]>("list_categories", undefined, mockCategories);
  },
  listCategoryItems() {
    return call<CategoryItem[]>("list_category_items", undefined, mockCategoryItems);
  },
  reorderCategories(categoryIds: string[]) {
    if (!isTauri) {
      const ordered = categoryIds
        .map((id, index) => {
          const category = mockCategories.find((item) => item.id === id);
          return category ? { ...category, sortOrder: index, updatedAt: new Date().toISOString() } : null;
        })
        .filter((item): item is Category => Boolean(item));
      mockCategories.splice(0, mockCategories.length, ...ordered);
      return Promise.resolve(structuredClone(mockCategories));
    }
    return invoke<Category[]>("reorder_categories", { categoryIds });
  },
  reorderCategoryItems(categoryId: string, itemIds: string[]) {
    if (!isTauri) {
      const timestamp = new Date().toISOString();
      const ordered = itemIds
        .map((id, index) => {
          const item = mockCategoryItems.find((entry) => entry.id === id && entry.categoryId === categoryId);
          return item ? { ...item, sortOrder: index, updatedAt: timestamp } : null;
        })
        .filter((item): item is CategoryItem => Boolean(item));
      const otherItems = mockCategoryItems.filter((item) => item.categoryId !== categoryId);
      mockCategoryItems.splice(0, mockCategoryItems.length, ...otherItems, ...ordered);
      return Promise.resolve(structuredClone(mockCategoryItems));
    }
    return invoke<CategoryItem[]>("reorder_category_items", { categoryId, itemIds });
  },
  createCategory(name: string, color: string) {
    return call<Category>("create_category", { name, color }, {
      id: crypto.randomUUID(),
      name,
      color,
      sortOrder: mockCategories.length,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    });
  },
  async createCategoryWithClip(name: string, color: string, clipId: string) {
    const clip = mockClips.find((item) => item.id === clipId) ?? mockClips[0];
    const category: Category = {
      id: crypto.randomUUID(),
      name,
      color,
      sortOrder: mockCategories.length,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };
    const fallback: CategoryWithItem = {
      category,
      item: {
        id: crypto.randomUUID(),
        categoryId: category.id,
        clipSnapshotId: clip.id,
        clipType: clip.clipType,
        contentHash: clip.contentHash,
        displayName: clip.displayName,
        previewText: clip.previewText,
        text: clip.text,
        sortOrder: 0,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        syncState: "local",
        isPinned: false,
      },
    };

    if (!isTauri) return structuredClone(fallback);

    try {
      return await invoke<CategoryWithItem>("create_category_with_clip", { name, color, clipId });
    } catch (unknownError) {
      if (!isMissingCommandError(unknownError, "create_category_with_clip")) throw unknownError;

      const category = await invoke<Category>("create_category", { name, color });
      const item = await invoke<CategoryItem>("add_clip_to_category", { clipId, categoryId: category.id });
      return { category, item };
    }
  },
  updateCategory(id: string, name: string, color: string) {
    return call<Category>("update_category", { id, name, color }, {
      id,
      name,
      color,
      sortOrder: 0,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    });
  },
  deleteCategory(id: string) {
    return call<void>("delete_category", { id });
  },
  addClipToCategory(clipId: string, categoryId: string) {
    const clip = mockClips.find((item) => item.id === clipId) ?? mockClips[0];
    return call<CategoryItem>("add_clip_to_category", { clipId, categoryId }, {
      id: crypto.randomUUID(),
      categoryId,
      clipSnapshotId: clip.id,
      clipType: clip.clipType,
      contentHash: clip.contentHash,
      displayName: clip.displayName,
      previewText: clip.previewText,
      text: clip.text,
      sortOrder: 0,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      syncState: "local",
      isPinned: false,
    });
  },
  removeCategoryItem(id: string) {
    return call<void>("remove_category_item", { id });
  },
  deleteClip(id: string) {
    return call<void>("delete_clip", { id });
  },
  renameClip(id: string, collection: "history" | "category", displayName: string | null) {
    const normalizedName = displayName?.trim() || null;
    const fallback =
      collection === "history"
        ? mockClips.find((item) => item.id === id)
        : mockCategoryItems.find((item) => item.id === id);
    return call<ClipItem | CategoryItem>(
      "rename_clip",
      { id, collection, displayName: normalizedName },
      fallback ? { ...fallback, displayName: normalizedName } : undefined,
    );
  },
  updateClipContent(id: string, collection: "history" | "category", text: string) {
    const fallback =
      collection === "history"
        ? mockClips.find((item) => item.id === id)
        : mockCategoryItems.find((item) => item.id === id);
    return call<ClipItem | CategoryItem>(
      "update_clip_content",
      { id, collection, text },
      fallback ? { ...fallback, text, previewText: previewText(text) } : undefined,
    );
  },
  copyClip(clipType: string, text: string) {
    if (!isTauri && navigator.clipboard && clipType !== "image") {
      return navigator.clipboard.writeText(text);
    }
    return call<void>("copy_clip", { clipType, text });
  },
  setListening(enabled: boolean) {
    return call<boolean>("set_listening", { enabled }, enabled);
  },
  setAppendCopyEnabled(enabled: boolean) {
    return call<boolean>("set_append_copy_enabled", { enabled }, enabled);
  },
  updateSettings(retentionDays: number) {
    return call<AppSettings>("update_settings", { retentionDays }, {
      shortcut: mockSnapshot.settings.shortcut,
      retentionDays,
      appendCopyTimeoutMinutes: mockSnapshot.settings.appendCopyTimeoutMinutes,
      panelOpenBehavior: mockSnapshot.settings.panelOpenBehavior,
      panelLayout: mockSnapshot.settings.panelLayout,
      ocrMode: mockSnapshot.settings.ocrMode,
      cloud: mockSnapshot.settings.cloud,
    });
  },
  updateAppendCopyTimeout(minutes: number) {
    return call<AppSettings>("update_append_copy_timeout", { minutes }, {
      shortcut: mockSnapshot.settings.shortcut,
      retentionDays: mockSnapshot.settings.retentionDays,
      appendCopyTimeoutMinutes: minutes,
      panelOpenBehavior: mockSnapshot.settings.panelOpenBehavior,
      panelLayout: mockSnapshot.settings.panelLayout,
      ocrMode: mockSnapshot.settings.ocrMode,
      cloud: mockSnapshot.settings.cloud,
    });
  },
  updateShortcut(shortcut: string) {
    return call<AppSettings>("update_shortcut", { shortcut }, {
      shortcut,
      retentionDays: mockSnapshot.settings.retentionDays,
      appendCopyTimeoutMinutes: mockSnapshot.settings.appendCopyTimeoutMinutes,
      panelOpenBehavior: mockSnapshot.settings.panelOpenBehavior,
      panelLayout: mockSnapshot.settings.panelLayout,
      ocrMode: mockSnapshot.settings.ocrMode,
      cloud: mockSnapshot.settings.cloud,
    });
  },
  setAppShortcutEnabled(enabled: boolean) {
    return call<boolean>("set_app_shortcut_enabled", { enabled }, enabled);
  },
  updatePanelOpenBehavior(behavior: AppSettings["panelOpenBehavior"]) {
    return call<AppSettings>("update_panel_open_behavior", { behavior }, {
      shortcut: mockSnapshot.settings.shortcut,
      retentionDays: mockSnapshot.settings.retentionDays,
      appendCopyTimeoutMinutes: mockSnapshot.settings.appendCopyTimeoutMinutes,
      panelOpenBehavior: behavior,
      panelLayout: mockSnapshot.settings.panelLayout,
      ocrMode: mockSnapshot.settings.ocrMode,
      cloud: mockSnapshot.settings.cloud,
    });
  },
  updatePanelLayout(layout: AppSettings["panelLayout"]) {
    return call<AppSettings>("update_panel_layout", { layout }, {
      shortcut: mockSnapshot.settings.shortcut,
      retentionDays: mockSnapshot.settings.retentionDays,
      appendCopyTimeoutMinutes: mockSnapshot.settings.appendCopyTimeoutMinutes,
      panelOpenBehavior: mockSnapshot.settings.panelOpenBehavior,
      panelLayout: layout,
      ocrMode: mockSnapshot.settings.ocrMode,
      cloud: mockSnapshot.settings.cloud,
    });
  },
  updateOcrMode(mode: OcrMode) {
    return call<AppSettings>("update_ocr_mode", { mode }, {
      shortcut: mockSnapshot.settings.shortcut,
      retentionDays: mockSnapshot.settings.retentionDays,
      appendCopyTimeoutMinutes: mockSnapshot.settings.appendCopyTimeoutMinutes,
      panelOpenBehavior: mockSnapshot.settings.panelOpenBehavior,
      panelLayout: mockSnapshot.settings.panelLayout,
      ocrMode: mode,
      cloud: mockSnapshot.settings.cloud,
    });
  },
  updateCloudSettings(apiAddress: string, apiKey: string) {
    return call<AppSettings>("update_cloud_settings", { apiAddress, apiKey }, {
      shortcut: mockSnapshot.settings.shortcut,
      retentionDays: 30,
      appendCopyTimeoutMinutes: mockSnapshot.settings.appendCopyTimeoutMinutes,
      panelOpenBehavior: mockSnapshot.settings.panelOpenBehavior,
      panelLayout: mockSnapshot.settings.panelLayout,
      ocrMode: mockSnapshot.settings.ocrMode,
      cloud: {
        apiAddress,
        apiKey,
        enabled: Boolean(apiAddress && apiKey),
        lastConnectedAt: new Date().toISOString(),
      },
    });
  },
  disableCloudSync() {
    return call<AppSettings>("disable_cloud_sync", undefined, {
      shortcut: mockSnapshot.settings.shortcut,
      retentionDays: 30,
      appendCopyTimeoutMinutes: mockSnapshot.settings.appendCopyTimeoutMinutes,
      panelOpenBehavior: mockSnapshot.settings.panelOpenBehavior,
      panelLayout: mockSnapshot.settings.panelLayout,
      ocrMode: mockSnapshot.settings.ocrMode,
      cloud: {
        apiAddress: "",
        apiKey: "",
        enabled: false,
        lastConnectedAt: null,
      },
    });
  },
  syncCloudNow() {
    return call<AppSnapshot>("sync_cloud_now", undefined, mockSnapshot);
  },
  syncCloudInBackground() {
    return call<void>("sync_cloud_in_background");
  },
  testCloudSettings(apiAddress: string, apiKey: string) {
    return call<boolean>("test_cloud_settings", { apiAddress, apiKey }, true);
  },
  appInfo() {
    return call<AppInfo>("get_app_info", undefined, fallbackAppInfo);
  },
  ocrInstallStatus() {
    return call<OcrInstallStatus>("get_ocr_install_status", undefined, fallbackOcrInstallStatus);
  },
  installOcrAssets() {
    return call<OcrInstallStatus>("install_ocr_assets", undefined, {
      ...fallbackOcrInstallStatus,
      installed: true,
      engineVersion: "5.5.0.20241111-portable",
      mode: fallbackOcrInstallStatus.mode,
      downloadedBytes: 37_557_099,
      totalBytes: 37_557_099,
    });
  },
  removeOcrAssets() {
    return call<OcrInstallStatus>("remove_ocr_assets", undefined, fallbackOcrInstallStatus);
  },
  recognizeImageText(imagePath: string) {
    return call<ImageOcrResult>("recognize_image_text", { imagePath }, {
      text: "iPaste 图片 OCR 测试 Select text from image 2026",
      engine: "mock",
      language: "chi_sim+eng",
      words: [],
    });
  },
  showPanel() {
    return call<void>("show_panel");
  },
  showSettings() {
    return call<void>("show_settings");
  },
  hidePanel() {
    return call<void>("hide_panel");
  },
  hideSettings() {
    return call<void>("hide_settings");
  },
  openAccessibilitySettings() {
    return call<void>("open_accessibility_settings");
  },
  applyClip(id: string, clipType: string, text: string) {
    return call<void>("apply_clip", { id, clipType, text });
  },
  closeClipViewer(label: string) {
    return call<void>("close_clip_viewer", { label });
  },
  openClipViewer(item: ClipViewItem, originalClipId: string) {
    const label = `clip-viewer-${Date.now()}-${crypto.randomUUID().slice(0, 8)}`;
    const payload: ClipViewerPayload = { label, originalClipId, item };
    localStorage.setItem(clipViewerStorageKey(label), JSON.stringify(payload));

    if (!isTauri) {
      window.open(
        `${window.location.origin}${window.location.pathname}?window=clip-viewer&label=${encodeURIComponent(label)}`,
        label,
        "width=840,height=620",
      );
      return Promise.resolve();
    }

    return invoke<void>("open_clip_viewer", {
      label,
      title: item.displayName?.trim() || item.previewText || "iPaste",
    });
  },
};

function isMissingCommandError(error: unknown, command: string) {
  const message = String(error).toLowerCase();
  return message.includes(command.toLowerCase()) && message.includes("command");
}

export function clipViewerStorageKey(label: string) {
  return `ipaste.clipViewer.${label}`;
}

function previewText(text: string) {
  return text.split(/\s+/).filter(Boolean).join(" ").slice(0, 180);
}
