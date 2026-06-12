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
};

export type CategoryWithItem = {
  category: Category;
  item: CategoryItem;
};

export type PanelOpenBehavior = "history" | "last_selected";
export type PanelLayout = "top" | "side";
export type OcrMode = "fast" | "best";

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
  cloud: CloudSettings;
};

export type AppInfo = {
  version: string;
};

export type OcrInstallStatus = {
  installed: boolean;
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
  phase: "fetchingManifest" | "downloading" | "completed" | string;
  fileName?: string | null;
  downloadedBytes: number;
  totalBytes: number;
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
};
