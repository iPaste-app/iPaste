export type ClipType = "text" | "link" | "color" | "image" | "file" | "html";

export type ClipItem = {
  id: string;
  clipType: ClipType;
  contentHash: string;
  displayName?: string | null;
  previewText: string;
  text: string;
  sourceApp?: string | null;
  lastCapturedAt: string;
  favoriteCount: number;
  isPinned: boolean;
  pinOrder?: number | null;
};

export type Category = {
  id: string;
  name: string;
  color: string;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
};

export type CategoryItem = {
  id: string;
  categoryId: string;
  clipSnapshotId: string;
  clipType: ClipType;
  contentHash: string;
  displayName?: string | null;
  previewText: string;
  text: string;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
  syncState: "local" | "syncing" | "synced" | "conflict";
  isPinned: boolean;
  pinOrder?: number | null;
};

export type CategoryWithItem = {
  category: Category;
  item: CategoryItem;
};

export type PanelOpenBehavior = "history" | "last_selected";
export type PanelLayout = "top" | "side";
export type OcrMode = "fast" | "best";
export type Language = "en" | "zh-CN" | "ja" | "ko" | "es" | "fr" | "de";

export type AppSnapshot = {
  clips: ClipItem[];
  hasMoreClips: boolean;
  clipTotalCount: number;
  categories: Category[];
  categoryItems: CategoryItem[];
  shortcut: string;
  isListening: boolean;
  isAppendCopyEnabled: boolean;
  settings: AppSettings;
};

export type ClipPage = {
  clips: ClipItem[];
  hasMore: boolean;
  totalCount: number;
  allCount: number;
};

export type AppSettings = {
  shortcut: string;
  retentionDays: number;
  appendCopyTimeoutMinutes: number;
  panelOpenBehavior: PanelOpenBehavior;
  panelLayout: PanelLayout;
  ocrMode: OcrMode;
  language: Language;
  cloud: CloudSettings;
};

export type AppInfo = {
  version: string;
};

export type StarPromptState = {
  status: "pending" | "snoozed" | "starred" | "retired";
  successfulPasteCount: number;
  snoozeCount: number;
  nextShowAt?: string | null;
  nextShowAfterPasteCount?: number | null;
  shouldShow: boolean;
};

export type OcrInstallStatus = {
  installed: boolean;
  needsRepair: boolean;
  hasResources: boolean;
  engineId: string;
  engineVersion?: string | null;
  mode: OcrMode;
  platform: string;
  manifestUrl: string;
  installDir: string;
  downloadedBytes: number;
  totalBytes: number;
  missingFiles: string[];
};

export type OcrInstallProgress = {
  mode: OcrMode;
  phase: "fetchingManifest" | "downloading" | "verifying" | "completed";
  fileName?: string | null;
  downloadedBytes: number;
  totalBytes: number;
  networkBytes: number;
};

export type ImageOcrResult = {
  text: string;
  engine: string;
  language: string;
  words: ImageOcrWord[];
};

export type ImageOcrWord = {
  text: string;
  left: number;
  top: number;
  width: number;
  height: number;
  confidence: number;
  blockIndex?: number;
  paragraphIndex?: number;
  lineIndex?: number;
  wordIndex?: number;
};

export type CloudSettings = {
  apiAddress: string;
  apiKey: string;
  enabled: boolean;
  lastConnectedAt?: string | null;
};

export type CapturedEvent = {
  clip: ClipItem;
  clipTotalCount: number;
  wasInserted: boolean;
};

export type ListeningChangedEvent = {
  isListening: boolean;
};

export type AppendCopyChangedEvent = {
  isEnabled: boolean;
};

export type SettingsChangedEvent = {
  settings: AppSettings;
};

export type ClipViewItem =
  | (ClipItem & { collection: "history" })
  | (CategoryItem & { collection: "category" });

export type ClipViewerPayload = {
  label: string;
  originalClipId: string;
  item: ClipViewItem;
};

export type ClipUpdatedEvent = {
  collection: "history" | "category";
  item: ClipItem | CategoryItem;
  mergedFromId?: string;
};
