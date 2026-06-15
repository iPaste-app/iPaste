<script setup lang="ts">
import { emit } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ChevronLeft,
  ChevronRight,
  ClipboardPaste,
  CornerDownLeft,
  Copy,
  Image as ImageIcon,
  LoaderCircle,
  Maximize2,
  Pin,
  PinOff,
  RotateCcw,
  RotateCw,
  ScanText,
  Save,
  X,
  ZoomIn,
  ZoomOut,
} from "lucide-vue-next";
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { clipImageSrc } from "../lib/clipMedia";
import { t } from "../i18n";
import { clipViewerStorageKey, ipasteApi } from "../lib/ipasteApi";
import { formatTime, textStats, typeLabel } from "../lib/format";
import type { ClipUpdatedEvent, ClipViewerPayload, ImageOcrResult, ImageOcrWord } from "../types";

type OcrSourceWord = ImageOcrWord & {
  sourceIndex: number;
};

type OcrSelectableWord = OcrSourceWord & {
  selectionIndex: number;
  lineKey: string;
  lineOrder: number;
};

type OcrLine = {
  key: string;
  text: string;
  left: number;
  top: number;
  width: number;
  height: number;
  order: number;
  words: OcrSelectableWord[];
};

type OcrSelectionRange = {
  startIndex: number;
  endIndex: number;
};

type OcrSelectionHighlight = {
  key: string;
  left: number;
  top: number;
  width: number;
  height: number;
};

const isTauri = "__TAURI_INTERNALS__" in window;
const isMacOs = /mac/i.test(navigator.platform) || /Mac OS/i.test(navigator.userAgent);
const payload = ref<ClipViewerPayload | null>(null);
const windowLabel = ref("");
const draftText = ref("");
const isPinned = ref(isTauri);
const error = ref<string | null>(null);
const selectionAction = ref<{ left: number; top: number; text: string; mode: "paste" | "copy" } | null>(null);
const editorElement = ref<HTMLTextAreaElement | null>(null);
const imageStageElement = ref<HTMLElement | null>(null);
const imageNaturalSize = ref({ width: 0, height: 0 });
const imageStageSize = ref({ width: 0, height: 0 });
const imageScale = ref(1);
const imageRotation = ref(0);
const imagePan = ref({ x: 0, y: 0 });
const imageViewMode = ref<"fit" | "actual" | "manual">("fit");
const isImageDragging = ref(false);
const isRecognizingImage = ref(false);
const imageOcrResult = ref<ImageOcrResult | null>(null);
const imageOcrError = ref<string | null>(null);
const imageOcrSelection = ref<OcrSelectionRange | null>(null);
const isImageOcrPanelCollapsed = ref(false);
const showClosePrompt = ref(false);
const isSavingBeforeClose = ref(false);
let selectionTimer: number | null = null;
let unlistenCloseRequested: (() => void) | null = null;
let isForceClosing = false;
let imageDragState: {
  pointerId: number;
  startX: number;
  startY: number;
  panX: number;
  panY: number;
} | null = null;
let imageOcrDragState: {
  pointerId: number;
  startIndex: number;
  captureElement: HTMLElement;
} | null = null;

const IMAGE_FIT_PADDING = 0;
const IMAGE_MIN_SCALE = 0.05;
const IMAGE_MAX_SCALE = 8;
const IMAGE_ZOOM_STEP = 1.2;

const item = computed(() => payload.value?.item);
const title = computed(() => {
  const current = item.value;
  if (!current) return t("viewer.titleFallback");
  return current.displayName?.trim() || t("clip.clipboardTitle", { type: typeLabel(current.clipType) });
});
const isImage = computed(() => item.value?.clipType === "image");
const imageSrc = computed(() => (item.value ? clipImageSrc(item.value) : ""));
const showImageOcrPanel = computed(() => isImage.value && (isRecognizingImage.value || Boolean(imageOcrResult.value) || Boolean(imageOcrError.value)));
const hasChanged = computed(() => Boolean(item.value && draftText.value !== item.value.text));
const stats = computed(() => (item.value ? textStats(draftText.value) : ""));
const lines = computed(() => draftText.value.split(/\r?\n/).length);
const normalizedImageRotation = computed(() => ((imageRotation.value % 360) + 360) % 360);
const isImageRotatedSideways = computed(() => normalizedImageRotation.value === 90 || normalizedImageRotation.value === 270);
const rotatedImageBaseSize = computed(() => {
  const { width, height } = imageNaturalSize.value;
  if (!width || !height) return { width: 0, height: 0 };
  return isImageRotatedSideways.value ? { width: height, height: width } : { width, height };
});
const imageDisplaySize = computed(() => ({
  width: rotatedImageBaseSize.value.width * imageScale.value,
  height: rotatedImageBaseSize.value.height * imageScale.value,
}));
const imagePanBounds = computed(() => ({
  x: Math.max(0, (imageDisplaySize.value.width - imageStageSize.value.width) / 2),
  y: Math.max(0, (imageDisplaySize.value.height - imageStageSize.value.height) / 2),
}));
const canPanImage = computed(() => imagePanBounds.value.x > 0 || imagePanBounds.value.y > 0);
const isImageActualSize = computed(() => Math.abs(imageScale.value - 1) < 0.005);
const imageZoomLabel = computed(() => `${Math.round(imageScale.value * 100)}%`);
const imageStyle = computed(() => {
  const { width, height } = imageNaturalSize.value;
  return {
    width: width ? `${width}px` : "auto",
    height: height ? `${height}px` : "auto",
    marginLeft: width ? `${-width / 2}px` : "0",
    marginTop: height ? `${-height / 2}px` : "0",
    transform: `rotate(${imageRotation.value}deg) scale(${imageScale.value})`,
  };
});
const imageFrameStyle = computed(() => ({
  transform: `translate(${imagePan.value.x}px, ${imagePan.value.y}px)`,
}));
const ocrTextLayerStyle = computed(() => {
  const { width, height } = imageNaturalSize.value;
  return {
    width: width ? `${width}px` : "0",
    height: height ? `${height}px` : "0",
    marginLeft: width ? `${-width / 2}px` : "0",
    marginTop: height ? `${-height / 2}px` : "0",
    transform: `rotate(${imageRotation.value}deg) scale(${imageScale.value})`,
  };
});
const imageOcrSummary = computed(() => {
  if (!imageOcrResult.value) return "";
  return t("viewer.ocrSummary", {
    count: imageOcrResult.value.words.length,
    language: imageOcrResult.value.language,
  });
});
const imageOcrLoadingText = computed(() =>
  isMacOs ? t("viewer.ocrLoading.macos") : t("viewer.ocrLoading.tesseract"),
);
const imageOcrLines = computed<OcrLine[]>(() => {
  const words = imageOcrResult.value?.words ?? [];
  if (!words.length) return [];

  const sourceWords = words
    .map((word, sourceIndex) => ({ ...word, sourceIndex }))
    .filter((word) => word.text.trim() && word.width > 0 && word.height > 0);
  const seeds = buildOcrLineSeeds(sourceWords);
  let selectionIndex = 0;

  return seeds.map((line, order) => {
    const ordered = [...line.words]
      .sort(compareOcrWordsInLine)
      .map((word) => ({
        ...word,
        lineKey: line.key,
        lineOrder: order,
        selectionIndex: selectionIndex++,
      }));
    const left = Math.min(...ordered.map((word) => word.left));
    const top = Math.min(...ordered.map((word) => word.top));
    const right = Math.max(...ordered.map((word) => word.left + word.width));
    const bottom = Math.max(...ordered.map((word) => word.top + word.height));
    return {
      key: line.key,
      text: joinOcrWords(ordered),
      left,
      top,
      width: right - left,
      height: bottom - top,
      order,
      words: ordered,
    };
  });
});
const imageOcrWords = computed(() => imageOcrLines.value.flatMap((line) => line.words));
const imageOcrSelectionBounds = computed(() => {
  const range = imageOcrSelection.value;
  if (!range) return null;
  return {
    start: Math.min(range.startIndex, range.endIndex),
    end: Math.max(range.startIndex, range.endIndex),
  };
});
const selectedImageOcrWordIndexes = computed(() => {
  const bounds = imageOcrSelectionBounds.value;
  if (!bounds) return new Set<number>();
  return new Set(
    imageOcrWords.value
      .filter((word) => word.selectionIndex >= bounds.start && word.selectionIndex <= bounds.end)
      .map((word) => word.selectionIndex),
  );
});
const imageOcrSelectionHighlights = computed<OcrSelectionHighlight[]>(() => {
  const selected = selectedImageOcrWordIndexes.value;
  if (!selected.size) return [];

  return imageOcrLines.value.flatMap((line) => {
    const selectedWords = line.words.filter((word) => selected.has(word.selectionIndex));
    if (!selectedWords.length) return [];

    const left = Math.min(...selectedWords.map((word) => word.left));
    const right = Math.max(...selectedWords.map((word) => word.left + word.width));
    const top = line.top;
    const bottom = line.top + line.height;
    return [{
      key: `${line.key}:${selectedWords[0].selectionIndex}:${selectedWords[selectedWords.length - 1].selectionIndex}`,
      left: Math.max(0, left - 2),
      top: Math.max(0, top - 2),
      width: right - left + 4,
      height: Math.max(1, bottom - top + 4),
    }];
  });
});
const imageOcrSelectionText = computed(() => {
  const selected = selectedImageOcrWordIndexes.value;
  if (!selected.size) return "";

  const lines = imageOcrLines.value
    .map((line) => line.words.filter((word) => selected.has(word.selectionIndex)))
    .filter((lineWords) => lineWords.length)
    .map(joinOcrWords);
  return lines.join("\n");
});
const imageOcrText = computed(() => {
  const lineText = imageOcrLines.value.map((line) => line.text).filter(Boolean).join("\n");
  return lineText || imageOcrResult.value?.text || "";
});
const canZoomOutImage = computed(() => imageScale.value > minimumImageScale() + 0.005);
const canZoomInImage = computed(() => imageScale.value < IMAGE_MAX_SCALE - 0.005);
const displayTime = computed(() => {
  const current = item.value;
  if (!current) return "";
  return current.collection === "history" ? current.lastCapturedAt : current.createdAt;
});

onMounted(async () => {
  loadPayload();
  if (isTauri) {
    try {
      isPinned.value = await getCurrentWindow().isAlwaysOnTop();
    } catch {
      isPinned.value = true;
    }
  }
  document.addEventListener("selectionchange", scheduleSelectionAction);
  document.addEventListener("keydown", handleViewerKeydown, true);
  window.addEventListener("resize", handleViewerResize);
  window.addEventListener("beforeunload", handleBeforeUnload);
  if (isTauri) {
    unlistenCloseRequested = await getCurrentWindow().onCloseRequested(async (event) => {
      event.preventDefault();
      if (isForceClosing) return;
      if (!hasChanged.value) {
        await forceCloseWindow();
        return;
      }

      requestClose();
    });
  }
  void nextTick(focusEditorAtStart);
});

onUnmounted(() => {
  document.removeEventListener("selectionchange", scheduleSelectionAction);
  document.removeEventListener("keydown", handleViewerKeydown, true);
  window.removeEventListener("resize", handleViewerResize);
  window.removeEventListener("beforeunload", handleBeforeUnload);
  clearSelectionTimer();
  unlistenCloseRequested?.();
  unlistenCloseRequested = null;
});

watch(draftText, () => {
  hideSelectionAction();
});

watch(imageSrc, () => {
  resetImageViewState();
});

function loadPayload() {
  const label = new URLSearchParams(window.location.search).get("label");
  if (!label) {
    error.value = t("viewer.payloadMissing");
    return;
  }
  windowLabel.value = label;

  const raw = localStorage.getItem(clipViewerStorageKey(label));
  if (!raw) {
    error.value = t("viewer.payloadExpired");
    return;
  }

  try {
    payload.value = JSON.parse(raw) as ClipViewerPayload;
    draftText.value = payload.value.item.text;
  } catch {
    error.value = t("viewer.payloadInvalid");
  }
}

function focusEditorAtStart() {
  const editor = editorElement.value;
  if (!editor) return;

  editor.focus();
  editor.setSelectionRange(0, 0);
  editor.scrollTop = 0;
  editor.scrollLeft = 0;
}

async function startWindowDrag(event: MouseEvent) {
  if (!isTauri || event.button !== 0) return;

  event.preventDefault();
  await getCurrentWindow().startDragging();
}

async function togglePinned() {
  isPinned.value = !isPinned.value;
  if (isTauri) {
    await getCurrentWindow().setAlwaysOnTop(isPinned.value);
  }
}

async function closeWindow() {
  if (hasChanged.value) {
    requestClose();
    return;
  }

  await forceCloseWindow();
}

function requestClose() {
  showClosePrompt.value = true;
  hideSelectionAction();
}

function cancelClose() {
  showClosePrompt.value = false;
}

async function saveAndClose() {
  if (!hasChanged.value) {
    await forceCloseWindow();
    return;
  }

  isSavingBeforeClose.value = true;
  try {
    await applyChanges();
  } finally {
    isSavingBeforeClose.value = false;
  }

  if (!hasChanged.value) {
    showClosePrompt.value = false;
    await forceCloseWindow();
  }
}

async function discardAndClose() {
  showClosePrompt.value = false;
  await forceCloseWindow();
}

async function forceCloseWindow() {
  isForceClosing = true;
  if (isTauri) {
    try {
      await ipasteApi.closeClipViewer(windowLabel.value || getCurrentWindow().label);
    } catch (unknownError) {
      isForceClosing = false;
      error.value = String(unknownError);
    }
    return;
  }

  window.close();
}

function handleBeforeUnload(event: BeforeUnloadEvent) {
  if (isForceClosing || !hasChanged.value) return;

  event.preventDefault();
  event.returnValue = "";
}

function handleViewerKeydown(event: KeyboardEvent) {
  if (
    isImage.value
    && event.key.toLowerCase() === "c"
    && (event.metaKey || event.ctrlKey)
    && !event.altKey
    && !event.shiftKey
    && imageOcrSelectionText.value.trim()
  ) {
    event.preventDefault();
    void ipasteApi.copyClip("text", imageOcrSelectionText.value);
    return;
  }

  if (
    event.defaultPrevented
    || event.key !== "Escape"
    || event.metaKey
    || event.ctrlKey
    || event.altKey
    || event.shiftKey
  ) {
    return;
  }

  event.preventDefault();
  if (showClosePrompt.value) {
    cancelClose();
    return;
  }

  void closeWindow();
}

function resetDraft() {
  if (!item.value) return;
  draftText.value = item.value.text;
  hideSelectionAction();
}

async function applyChanges() {
  if (!item.value || !hasChanged.value) return;

  try {
    const next = await ipasteApi.updateClipContent(item.value.id, item.value.collection, draftText.value);
    const nextItem = { ...next, collection: item.value.collection } as typeof item.value;
    payload.value = {
      ...payload.value!,
      item: nextItem,
    };
    localStorage.setItem(clipViewerStorageKey(payload.value.label), JSON.stringify(payload.value));
    draftText.value = next.text;
    if (isTauri) {
      await emit<ClipUpdatedEvent>("ipaste://clip-updated", {
        collection: item.value.collection,
        item: next,
      });
    }
  } catch (unknownError) {
    error.value = String(unknownError);
  }
}

async function pasteDraft() {
  if (!payload.value || !item.value) return;
  await pasteFromViewer(draftText.value);
}

async function pasteSelection() {
  if (!payload.value || !item.value || !selectionAction.value?.text) return;
  const selectedText = selectionAction.value.text;
  const mode = selectionAction.value.mode;
  hideSelectionAction();
  if (mode === "copy") {
    await ipasteApi.copyClip("text", selectedText);
    return;
  }
  await pasteFromViewer(selectedText);
}

async function pasteFromViewer(text: string) {
  if (!payload.value || !item.value) return;

  const viewerWindow = isTauri ? getCurrentWindow() : null;
  if (viewerWindow) {
    await viewerWindow.hide();
  }

  try {
    await ipasteApi.applyClip(payload.value.originalClipId, item.value.clipType, text);
  } finally {
    if (viewerWindow) {
      await viewerWindow.show();
      await viewerWindow.setAlwaysOnTop(isPinned.value);
      await viewerWindow.setFocus();
    }
  }
}

function scheduleSelectionAction() {
  clearSelectionTimer();
  selectionTimer = window.setTimeout(updateSelectionAction, 80);
}

function updateSelectionAction() {
  selectionTimer = null;
  if (isImage.value && imageOcrSelectionText.value.trim()) {
    updateImageOcrSelectionAction();
    return;
  }

  const textarea = editorElement.value;
  if (!textarea || document.activeElement !== textarea) {
    hideSelectionAction();
    return;
  }

  const selectedText = draftText.value.slice(textarea.selectionStart, textarea.selectionEnd);
  if (!selectedText.trim()) {
    hideSelectionAction();
    return;
  }

  const coords = selectionCoordinates(textarea, textarea.selectionEnd);
  const fallbackRect = textarea.getBoundingClientRect();
  selectionAction.value = {
    left: Math.min(fallbackRect.right - 128, Math.max(fallbackRect.left + 16, coords.left - 48)),
    top: Math.min(window.innerHeight - 56, coords.top + coords.height + 8),
    text: selectedText,
    mode: "paste",
  };
}

function buildOcrLineSeeds(words: OcrSourceWord[]) {
  if (!words.length) return [];

  const hasStructuredLines = words.every((word) => (
    Number.isFinite(word.blockIndex)
    && Number.isFinite(word.paragraphIndex)
    && Number.isFinite(word.lineIndex)
  ));

  if (hasStructuredLines) {
    const groups = new Map<string, {
      key: string;
      words: OcrSourceWord[];
      blockIndex: number;
      paragraphIndex: number;
      lineIndex: number;
      firstSourceIndex: number;
      top: number;
      left: number;
    }>();

    for (const word of words) {
      const blockIndex = word.blockIndex ?? 0;
      const paragraphIndex = word.paragraphIndex ?? 0;
      const lineIndex = word.lineIndex ?? 0;
      const key = `${blockIndex}:${paragraphIndex}:${lineIndex}`;
      const group = groups.get(key);
      if (group) {
        group.words.push(word);
        group.firstSourceIndex = Math.min(group.firstSourceIndex, word.sourceIndex);
        group.top = Math.min(group.top, word.top);
        group.left = Math.min(group.left, word.left);
      } else {
        groups.set(key, {
          key,
          words: [word],
          blockIndex,
          paragraphIndex,
          lineIndex,
          firstSourceIndex: word.sourceIndex,
          top: word.top,
          left: word.left,
        });
      }
    }

    return [...groups.values()].sort((a, b) => (
      a.blockIndex - b.blockIndex
      || a.paragraphIndex - b.paragraphIndex
      || a.lineIndex - b.lineIndex
      || a.firstSourceIndex - b.firstSourceIndex
      || a.top - b.top
      || a.left - b.left
    ));
  }

  const sorted = [...words].sort((a, b) => (a.top - b.top) || (a.left - b.left) || (a.sourceIndex - b.sourceIndex));
  const lines: Array<{
    key: string;
    words: OcrSourceWord[];
    top: number;
    bottom: number;
    left: number;
  }> = [];

  for (const word of sorted) {
    const centerY = word.top + word.height / 2;
    const bestLine = lines
      .map((line) => ({
        line,
        distance: centerY < line.top ? line.top - centerY : Math.max(0, centerY - line.bottom),
      }))
      .sort((a, b) => a.distance - b.distance)[0];
    const tolerance = Math.max(4, word.height * 0.55);

    if (bestLine && bestLine.distance <= tolerance) {
      bestLine.line.words.push(word);
      bestLine.line.top = Math.min(bestLine.line.top, word.top);
      bestLine.line.bottom = Math.max(bestLine.line.bottom, word.top + word.height);
      bestLine.line.left = Math.min(bestLine.line.left, word.left);
    } else {
      lines.push({
        key: `geometry:${lines.length}`,
        words: [word],
        top: word.top,
        bottom: word.top + word.height,
        left: word.left,
      });
    }
  }

  return lines.sort((a, b) => (a.top - b.top) || (a.left - b.left));
}

function compareOcrWordsInLine(a: OcrSourceWord, b: OcrSourceWord) {
  if (Number.isFinite(a.wordIndex) && Number.isFinite(b.wordIndex) && a.wordIndex !== b.wordIndex) {
    return (a.wordIndex ?? 0) - (b.wordIndex ?? 0);
  }
  return (a.left - b.left) || (a.sourceIndex - b.sourceIndex);
}

function joinOcrWords(words: Array<Pick<OcrSelectableWord, "text">>) {
  return words.reduce((result, word) => {
    const text = word.text.trim();
    if (!text) return result;
    if (!result) return text;
    const previous = result[result.length - 1] ?? "";
    const separator = shouldInsertOcrSpace(previous, text[0]) ? " " : "";
    return `${result}${separator}${text}`;
  }, "");
}

function shouldInsertOcrSpace(previous: string, next: string) {
  if (!previous || !next) return false;
  const cjkPattern = /[\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff]/u;
  if (cjkPattern.test(previous) || cjkPattern.test(next)) return false;
  if (/^[,.;:!?%)}\]，。；：！？、）】》]/u.test(next)) return false;
  if (/[(\[{$（【《]$/u.test(previous)) return false;
  return /[A-Za-z0-9)\]}]$/u.test(previous) && /^[A-Za-z0-9({[]/u.test(next);
}

function startImageOcrSelection(event: PointerEvent, selectionIndex: number) {
  if (event.button !== 0) return;

  event.preventDefault();
  event.stopPropagation();
  endImageDrag();

  const captureElement = event.currentTarget as HTMLElement;
  imageOcrDragState = {
    pointerId: event.pointerId,
    startIndex: selectionIndex,
    captureElement,
  };
  imageOcrSelection.value = {
    startIndex: selectionIndex,
    endIndex: selectionIndex,
  };
  captureElement.setPointerCapture(event.pointerId);
  updateImageOcrSelectionAction();
}

function moveImageOcrSelection(event: PointerEvent) {
  if (!imageOcrDragState || event.pointerId !== imageOcrDragState.pointerId) return;

  event.preventDefault();
  event.stopPropagation();

  const selectionIndex = imageOcrWordIndexFromPoint(event, true);
  if (selectionIndex === null) return;
  imageOcrSelection.value = {
    startIndex: imageOcrDragState.startIndex,
    endIndex: selectionIndex,
  };
  updateImageOcrSelectionAction();
}

function finishImageOcrSelection(event: PointerEvent) {
  if (!imageOcrDragState || event.pointerId !== imageOcrDragState.pointerId) return;

  event.preventDefault();
  event.stopPropagation();

  const selectionIndex = imageOcrWordIndexFromPoint(event, true);
  if (selectionIndex !== null) {
    imageOcrSelection.value = {
      startIndex: imageOcrDragState.startIndex,
      endIndex: selectionIndex,
    };
  }

  if (imageOcrDragState.captureElement.hasPointerCapture(event.pointerId)) {
    imageOcrDragState.captureElement.releasePointerCapture(event.pointerId);
  }
  endImageOcrSelection();
  updateImageOcrSelectionAction();
}

function endImageOcrSelection() {
  imageOcrDragState = null;
}

function updateImageOcrSelectionAction() {
  const selectedText = imageOcrSelectionText.value;
  if (!selectedText.trim()) {
    hideSelectionAction();
    return;
  }

  const selectedIndexes = selectedImageOcrWordIndexes.value;
  const wordElements = imageStageElement.value?.querySelectorAll<HTMLElement>(".viewer-image-ocr-word") ?? [];
  const rects = [...wordElements]
    .filter((element) => selectedIndexes.has(Number(element.dataset.ocrWordIndex)))
    .map((element) => element.getBoundingClientRect())
    .filter((rect) => rect.width || rect.height);
  const rect = unionDomRects(rects);
  if (!rect) {
    hideSelectionAction();
    return;
  }

  selectionAction.value = {
    left: Math.min(window.innerWidth - 132, Math.max(16, rect.right - 112)),
    top: Math.min(window.innerHeight - 56, rect.bottom + 8),
    text: selectedText,
    mode: "copy",
  };
}

function unionDomRects(rects: DOMRect[]) {
  if (!rects.length) return null;

  const left = Math.min(...rects.map((rect) => rect.left));
  const top = Math.min(...rects.map((rect) => rect.top));
  const right = Math.max(...rects.map((rect) => rect.right));
  const bottom = Math.max(...rects.map((rect) => rect.bottom));
  return { left, top, right, bottom };
}

function imageOcrWordIndexFromPoint(event: PointerEvent, allowNearest: boolean) {
  const point = clientPointToImagePoint(event.clientX, event.clientY);
  if (!point) return null;

  const scale = Math.max(0.001, imageScale.value);
  const tolerance = Math.max(2, Math.min(18, 6 / scale));
  const exactWord = imageOcrWords.value.find((word) => (
    point.x >= word.left - tolerance
    && point.x <= word.left + word.width + tolerance
    && point.y >= word.top - tolerance
    && point.y <= word.top + word.height + tolerance
  ));
  if (exactWord) return exactWord.selectionIndex;
  if (!allowNearest) return null;

  return nearestImageOcrWordIndex(point.x, point.y);
}

function clientPointToImagePoint(clientX: number, clientY: number) {
  const stage = imageStageElement.value;
  const { width, height } = imageNaturalSize.value;
  if (!stage || !width || !height || imageScale.value <= 0) return null;

  const rect = stage.getBoundingClientRect();
  const centeredX = clientX - rect.left - rect.width / 2 - imagePan.value.x;
  const centeredY = clientY - rect.top - rect.height / 2 - imagePan.value.y;
  const radians = -normalizedImageRotation.value * Math.PI / 180;
  const rotatedX = centeredX * Math.cos(radians) - centeredY * Math.sin(radians);
  const rotatedY = centeredX * Math.sin(radians) + centeredY * Math.cos(radians);

  return {
    x: rotatedX / imageScale.value + width / 2,
    y: rotatedY / imageScale.value + height / 2,
  };
}

function nearestImageOcrWordIndex(x: number, y: number) {
  const lines = imageOcrLines.value;
  if (!lines.length) return null;

  if (y <= lines[0].top) return lines[0].words[0]?.selectionIndex ?? null;

  const lastLine = lines[lines.length - 1];
  if (lastLine && y >= lastLine.top + lastLine.height) {
    return lastLine.words[lastLine.words.length - 1]?.selectionIndex ?? null;
  }

  const line = lines
    .map((entry) => ({
      line: entry,
      distance: y < entry.top ? entry.top - y : Math.max(0, y - entry.top - entry.height),
    }))
    .sort((a, b) => a.distance - b.distance)[0]?.line;
  if (!line) return null;

  const words = line.words;
  if (!words.length) return null;
  const firstWord = words[0];
  const lastWord = words[words.length - 1];
  if (x <= firstWord.left + firstWord.width / 2) return firstWord.selectionIndex;
  if (x >= lastWord.left + lastWord.width / 2) return lastWord.selectionIndex;

  return words
    .map((word) => ({
      word,
      distance: Math.abs(x - (word.left + word.width / 2)),
    }))
    .sort((a, b) => a.distance - b.distance)[0]?.word.selectionIndex ?? null;
}

function selectionCoordinates(textarea: HTMLTextAreaElement, position: number) {
  const rect = textarea.getBoundingClientRect();
  const style = window.getComputedStyle(textarea);
  const mirror = document.createElement("div");
  const marker = document.createElement("span");

  [
    "boxSizing",
    "borderTopWidth",
    "borderRightWidth",
    "borderBottomWidth",
    "borderLeftWidth",
    "fontFamily",
    "fontSize",
    "fontWeight",
    "letterSpacing",
    "lineHeight",
    "paddingTop",
    "paddingRight",
    "paddingBottom",
    "paddingLeft",
    "textTransform",
    "textIndent",
    "wordSpacing",
    "wordBreak",
  ].forEach((property) => {
    mirror.style.setProperty(property, style.getPropertyValue(property));
  });

  Object.assign(mirror.style, {
    position: "fixed",
    left: `${rect.left - textarea.scrollLeft}px`,
    top: `${rect.top - textarea.scrollTop}px`,
    width: `${textarea.offsetWidth}px`,
    height: "auto",
    minHeight: "0",
    overflow: "hidden",
    overflowWrap: "break-word",
    pointerEvents: "none",
    visibility: "hidden",
    whiteSpace: "pre-wrap",
    zIndex: "-1",
  });

  mirror.append(
    document.createTextNode(draftText.value.slice(0, position)),
    marker,
    document.createTextNode(draftText.value.slice(position) || "\u200b"),
  );
  marker.textContent = "\u200b";
  document.body.appendChild(mirror);
  const markerRect = marker.getBoundingClientRect();
  document.body.removeChild(mirror);

  return {
    left: markerRect.left,
    top: markerRect.top,
    height: markerRect.height || Number.parseFloat(style.lineHeight) || 22,
  };
}

function hideSelectionAction() {
  selectionAction.value = null;
}

function clearImageTextSelection() {
  if (!isImage.value) return;
  imageOcrSelection.value = null;
  endImageOcrSelection();
  hideSelectionAction();
}

function clearSelectionTimer() {
  if (selectionTimer === null) return;
  window.clearTimeout(selectionTimer);
  selectionTimer = null;
}

function handleViewerResize() {
  hideSelectionAction();
  clearImageTextSelection();
  updateImageStageSize();
  if (isImage.value && imageViewMode.value === "fit") {
    fitImageToStage();
    return;
  }

  clampImagePan();
}

function updateImageStageSize() {
  const stage = imageStageElement.value;
  if (!stage) return;

  const rect = stage.getBoundingClientRect();
  imageStageSize.value = {
    width: rect.width,
    height: rect.height,
  };
}

function handleImageLoad(event: Event) {
  const target = event.currentTarget as HTMLImageElement;
  imageNaturalSize.value = {
    width: target.naturalWidth,
    height: target.naturalHeight,
  };
  void nextTick(() => {
    updateImageStageSize();
    fitImageToStage();
  });
}

function resetImageViewState() {
  imageNaturalSize.value = { width: 0, height: 0 };
  imageStageSize.value = { width: 0, height: 0 };
  imageScale.value = 1;
  imageRotation.value = 0;
  imagePan.value = { x: 0, y: 0 };
  imageViewMode.value = "fit";
  imageOcrResult.value = null;
  imageOcrError.value = null;
  isImageOcrPanelCollapsed.value = false;
  clearImageTextSelection();
  endImageDrag();
}

async function recognizeImageText() {
  if (!item.value || !isImage.value || isRecognizingImage.value) return;

  isRecognizingImage.value = true;
  imageOcrError.value = null;
  isImageOcrPanelCollapsed.value = false;
  clearImageTextSelection();
  try {
    imageOcrResult.value = await ipasteApi.recognizeImageText(item.value.text);
  } catch (unknownError) {
    imageOcrError.value = String(unknownError);
  } finally {
    isRecognizingImage.value = false;
  }
}

async function pasteImageOcrText() {
  const text = imageOcrText.value;
  if (!text.trim()) return;
  await ipasteApi.copyClip("text", text);
}

function toggleImageOcrPanel() {
  isImageOcrPanelCollapsed.value = !isImageOcrPanelCollapsed.value;
}

function fitImageToStage() {
  clearImageTextSelection();
  imageScale.value = fitImageScale();
  imagePan.value = { x: 0, y: 0 };
  imageViewMode.value = "fit";
}

function showImageActualSize() {
  clearImageTextSelection();
  updateImageStageSize();
  imageScale.value = 1;
  imagePan.value = { x: 0, y: 0 };
  imageViewMode.value = "actual";
  clampImagePan();
}

function zoomImageIn() {
  setImageScale(imageScale.value * IMAGE_ZOOM_STEP);
}

function zoomImageOut() {
  setImageScale(imageScale.value / IMAGE_ZOOM_STEP);
}

function setImageScale(nextScale: number, anchor?: { x: number; y: number }) {
  updateImageStageSize();
  const currentScale = imageScale.value;
  const clampedScale = clamp(nextScale, minimumImageScale(), IMAGE_MAX_SCALE);
  if (!Number.isFinite(clampedScale) || Math.abs(clampedScale - currentScale) < 0.001) return;
  clearImageTextSelection();

  if (anchor) {
    const stage = imageStageElement.value;
    const rect = stage?.getBoundingClientRect();
    if (rect) {
      const anchorX = anchor.x - rect.left - rect.width / 2;
      const anchorY = anchor.y - rect.top - rect.height / 2;
      const ratio = clampedScale / currentScale;
      imagePan.value = {
        x: anchorX - (anchorX - imagePan.value.x) * ratio,
        y: anchorY - (anchorY - imagePan.value.y) * ratio,
      };
    }
  }

  imageScale.value = clampedScale;
  imageViewMode.value = isImageActualSize.value ? "actual" : "manual";
  clampImagePan();
}

function rotateImageClockwise() {
  clearImageTextSelection();
  const shouldRefit = imageViewMode.value === "fit";
  imageRotation.value = normalizedImageRotation.value + 90;
  if (shouldRefit) {
    void nextTick(fitImageToStage);
    return;
  }

  void nextTick(clampImagePan);
}

function handleImageWheel(event: WheelEvent) {
  event.preventDefault();
  const direction = event.deltaY < 0 ? 1 : -1;
  const factor = direction > 0 ? IMAGE_ZOOM_STEP : 1 / IMAGE_ZOOM_STEP;
  setImageScale(imageScale.value * factor, { x: event.clientX, y: event.clientY });
}

function startImagePan(event: PointerEvent) {
  if (event.button !== 0) return;
  clearImageTextSelection();
  if (!canPanImage.value) return;

  imageDragState = {
    pointerId: event.pointerId,
    startX: event.clientX,
    startY: event.clientY,
    panX: imagePan.value.x,
    panY: imagePan.value.y,
  };
  isImageDragging.value = true;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  event.preventDefault();
}

function moveImagePan(event: PointerEvent) {
  if (!imageDragState || event.pointerId !== imageDragState.pointerId) return;

  imagePan.value = constrainImagePan({
    x: imageDragState.panX + event.clientX - imageDragState.startX,
    y: imageDragState.panY + event.clientY - imageDragState.startY,
  });
}

function finishImagePan(event: PointerEvent) {
  if (!imageDragState || event.pointerId !== imageDragState.pointerId) return;

  const target = event.currentTarget as HTMLElement;
  if (target.hasPointerCapture(event.pointerId)) {
    target.releasePointerCapture(event.pointerId);
  }
  endImageDrag();
}

function endImageDrag() {
  imageDragState = null;
  isImageDragging.value = false;
}

function clampImagePan() {
  imagePan.value = constrainImagePan(imagePan.value);
}

function constrainImagePan(nextPan: { x: number; y: number }) {
  const bounds = imagePanBounds.value;
  return {
    x: clamp(nextPan.x, -bounds.x, bounds.x),
    y: clamp(nextPan.y, -bounds.y, bounds.y),
  };
}

function fitImageScale() {
  const { width, height } = rotatedImageBaseSize.value;
  const stageWidth = imageStageSize.value.width;
  const stageHeight = imageStageSize.value.height;
  if (!width || !height || !stageWidth || !stageHeight) return 1;

  const availableWidth = Math.max(1, stageWidth - IMAGE_FIT_PADDING);
  const availableHeight = Math.max(1, stageHeight - IMAGE_FIT_PADDING);
  return clamp(Math.min(availableWidth / width, availableHeight / height, 1), IMAGE_MIN_SCALE, 1);
}

function minimumImageScale() {
  return Math.min(IMAGE_MIN_SCALE, fitImageScale());
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}
</script>

<template>
  <main class="clip-viewer-shell">
    <header class="clip-viewer-toolbar" :class="{ 'clip-viewer-toolbar-image': isImage }">
      <button
        type="button"
        class="viewer-icon-button"
        :class="{ 'viewer-icon-button-active': isPinned }"
        :aria-label="isPinned ? t('viewer.unpin') : t('viewer.pin')"
        :data-tooltip="isPinned ? t('viewer.unpin') : t('viewer.pin')"
        @click="togglePinned"
      >
        <PinOff v-if="isPinned" class="size-4" />
        <Pin v-else class="size-4" />
      </button>

      <div class="clip-viewer-drag-zone min-w-0 flex-1" @mousedown="startWindowDrag">
        <h1 class="truncate text-base font-semibold text-slate-950">{{ title }}</h1>
        <p v-if="item" class="truncate text-xs text-slate-500">
          {{ typeLabel(item.clipType) }} · {{ formatTime(displayTime) }}
        </p>
      </div>

      <div v-if="isImage" class="viewer-image-toolbox" role="toolbar" :aria-label="t('viewer.imageToolbar')" @pointerdown.stop @wheel.stop>
        <button
          type="button"
          class="viewer-icon-button"
          :disabled="!canZoomOutImage"
          :aria-label="t('viewer.zoomOut')"
          :data-tooltip="t('viewer.zoomOut')"
          @click="zoomImageOut"
        >
          <ZoomOut class="size-4" />
        </button>
        <button
          type="button"
          class="viewer-icon-button"
          :disabled="!canZoomInImage"
          :aria-label="t('viewer.zoomIn')"
          :data-tooltip="t('viewer.zoomIn')"
          @click="zoomImageIn"
        >
          <ZoomIn class="size-4" />
        </button>
        <button
          type="button"
          class="viewer-icon-button"
          :class="{ 'viewer-icon-button-active': isImageActualSize }"
          :aria-label="t('viewer.actualSize')"
          :data-tooltip="t('viewer.actualSize')"
          @click="showImageActualSize"
        >
          <Maximize2 class="size-4" />
        </button>
        <button
          type="button"
          class="viewer-icon-button"
          :aria-label="t('viewer.rotateClockwise')"
          :data-tooltip="t('viewer.rotateClockwise')"
          @click="rotateImageClockwise"
        >
          <RotateCw class="size-4" />
        </button>
        <button
          type="button"
          class="viewer-image-zoom-label"
          :aria-label="t('viewer.restore100')"
          :data-tooltip="t('viewer.restore100')"
          @click="showImageActualSize"
        >
          {{ imageZoomLabel }}
        </button>
        <button
          type="button"
          class="viewer-icon-button"
          :class="{ 'viewer-icon-button-active': Boolean(imageOcrResult) }"
          :disabled="isRecognizingImage"
          :aria-label="t('viewer.recognizeText')"
          :data-tooltip="t('viewer.recognizeText')"
          @click="recognizeImageText"
        >
          <LoaderCircle v-if="isRecognizingImage" class="size-4 update-spin" />
          <ScanText v-else class="size-4" />
        </button>
      </div>

      <button
        v-if="!isImage"
        type="button"
        class="viewer-action-button"
        :disabled="!hasChanged"
        @click="resetDraft"
      >
        <RotateCcw class="size-4" />
        <span>{{ t("viewer.reset") }}</span>
      </button>

      <button
        v-if="!isImage"
        type="button"
        class="viewer-action-button viewer-action-button-primary"
        :disabled="!hasChanged"
        @click="applyChanges"
      >
        <Save class="size-4" />
        <span>{{ t("viewer.applyChanges") }}</span>
      </button>

      <button type="button" class="viewer-icon-button" :aria-label="t('viewer.closeWindow')" :data-tooltip="t('viewer.closeWindow')" @click="closeWindow">
        <X class="size-4" />
      </button>
    </header>

    <div v-if="error" class="viewer-error">{{ error }}</div>

    <section
      v-else-if="item"
      class="clip-viewer-content"
      :class="{
        'clip-viewer-content-image': isImage,
      }"
    >
      <template v-if="isImage">
        <div
          ref="imageStageElement"
          class="viewer-image-stage"
          :class="{
            'viewer-image-stage-pannable': canPanImage,
            'viewer-image-stage-dragging': isImageDragging,
            'viewer-image-stage-recognizing': isRecognizingImage,
          }"
          @wheel="handleImageWheel"
          @pointerdown="startImagePan"
          @pointermove="moveImagePan"
          @pointerup="finishImagePan"
          @pointercancel="finishImagePan"
          @lostpointercapture="endImageDrag"
        >
          <div class="viewer-image-frame" :style="imageFrameStyle">
            <img
              :src="imageSrc"
              :style="imageStyle"
              draggable="false"
              :alt="t('common.imagePreviewAlt')"
              @load="handleImageLoad"
            />
            <div
              v-if="imageOcrLines.length"
              class="viewer-image-ocr-layer"
              :style="ocrTextLayerStyle"
            >
              <span
                v-for="highlight in imageOcrSelectionHighlights"
                :key="highlight.key"
                class="viewer-image-ocr-highlight"
                :style="{
                  left: `${highlight.left}px`,
                  top: `${highlight.top}px`,
                  width: `${highlight.width}px`,
                  height: `${highlight.height}px`,
                }"
              />
              <span
                v-for="line in imageOcrLines"
                :key="line.key"
                class="viewer-image-ocr-line"
                :style="{
                  left: `${line.left}px`,
                  top: `${line.top}px`,
                  width: `${line.width}px`,
                  height: `${line.height}px`,
                  fontSize: `${Math.max(10, line.height * 0.84)}px`,
                }"
                aria-hidden="true"
              >
                {{ line.text }}
              </span>
              <button
                v-for="word in imageOcrWords"
                :key="`${word.lineKey}:${word.selectionIndex}`"
                type="button"
                class="viewer-image-ocr-word"
                :class="{ 'viewer-image-ocr-word-selected': selectedImageOcrWordIndexes.has(word.selectionIndex) }"
                :data-ocr-word-index="word.selectionIndex"
                :aria-label="word.text"
                :style="{
                  left: `${word.left}px`,
                  top: `${word.top}px`,
                  width: `${word.width}px`,
                  height: `${word.height}px`,
                }"
                @pointerdown="startImageOcrSelection($event, word.selectionIndex)"
                @pointermove="moveImageOcrSelection"
                @pointerup="finishImageOcrSelection"
                @pointercancel="finishImageOcrSelection"
                @lostpointercapture="endImageOcrSelection"
              />
            </div>
          </div>

          <div v-if="isRecognizingImage" class="viewer-image-scan-mask" aria-hidden="true">
            <span />
          </div>

          <aside
            v-if="showImageOcrPanel"
            class="viewer-image-ocr-panel"
            :class="{ 'viewer-image-ocr-panel-collapsed': isImageOcrPanelCollapsed }"
            @wheel.stop
          >
            <button
              type="button"
              class="viewer-image-ocr-toggle"
              :aria-label="isImageOcrPanelCollapsed ? t('viewer.expandOcr') : t('viewer.collapseOcr')"
              :data-tooltip="isImageOcrPanelCollapsed ? t('viewer.expandOcr') : t('viewer.collapseOcr')"
              @pointerdown.stop
              @click="toggleImageOcrPanel"
            >
              <ChevronLeft v-if="isImageOcrPanelCollapsed" class="size-4" />
              <ChevronRight v-else class="size-4" />
            </button>

            <div class="viewer-image-ocr-panel-body" @pointerdown.stop @wheel.stop>
              <div class="viewer-image-ocr-heading">
                <div class="min-w-0">
                  <h2>{{ t("viewer.ocrTitle") }}</h2>
                  <p v-if="imageOcrResult">{{ imageOcrSummary }}</p>
                  <p v-else-if="isRecognizingImage">{{ t("viewer.ocrRecognizing") }}</p>
                  <p v-else>{{ t("viewer.ocrFailed") }}</p>
                </div>
                <button
                  v-if="imageOcrResult?.text"
                  type="button"
                  class="viewer-paste-button"
                  @click="pasteImageOcrText"
                >
                  <Copy class="size-4" />
                  <span>{{ t("viewer.copyText") }}</span>
                </button>
              </div>

              <p v-if="imageOcrError" class="viewer-image-ocr-error">{{ imageOcrError }}</p>
              <p v-else-if="isRecognizingImage" class="viewer-image-ocr-loading">{{ imageOcrLoadingText }}</p>
              <textarea
                v-else-if="imageOcrResult"
                class="viewer-image-ocr-text subtle-scrollbar"
                :value="imageOcrText"
                readonly
                spellcheck="false"
                @focus="clearImageTextSelection"
                @pointerdown="clearImageTextSelection"
              />
            </div>
          </aside>
        </div>
      </template>

      <textarea
        v-else
        ref="editorElement"
        v-model="draftText"
        class="viewer-editor subtle-scrollbar"
        spellcheck="false"
        @mouseup="scheduleSelectionAction"
        @keyup="scheduleSelectionAction"
        @blur="hideSelectionAction"
      />

      <button
        v-if="selectionAction"
        type="button"
        class="selection-paste-button"
        :style="{ left: `${selectionAction.left}px`, top: `${selectionAction.top}px` }"
        @mousedown.prevent
        @click="pasteSelection"
      >
        <Copy v-if="selectionAction.mode === 'copy'" class="size-3.5" />
        <ClipboardPaste v-else class="size-3.5" />
        <span>{{ selectionAction.mode === "copy" ? t("viewer.copySelection") : t("viewer.pasteSelection") }}</span>
      </button>
    </section>

    <footer v-if="item" class="clip-viewer-footer">
      <span>{{ isImage ? item.previewText : stats }}</span>
      <span v-if="!isImage">{{ t("common.lineCount", { count: lines }) }}</span>
      <button type="button" class="viewer-paste-button" @click="pasteDraft">
        <ImageIcon v-if="isImage" class="size-4" />
        <CornerDownLeft v-else class="size-4" />
        <span>{{ isImage ? t("viewer.pasteImage") : t("viewer.pasteCurrent") }}</span>
      </button>
    </footer>

    <div v-if="showClosePrompt" class="viewer-close-backdrop" @mousedown.self="cancelClose">
      <section class="viewer-close-dialog" role="alertdialog" aria-modal="true" aria-labelledby="viewer-close-title">
        <h2 id="viewer-close-title">{{ t("viewer.saveChangesTitle") }}</h2>
        <p>{{ t("viewer.saveChangesDescription") }}</p>
        <div class="viewer-close-actions">
          <button type="button" class="viewer-action-button" :disabled="isSavingBeforeClose" @click="cancelClose">
            <span>{{ t("common.cancel") }}</span>
          </button>
          <button type="button" class="viewer-action-button viewer-action-button-danger" :disabled="isSavingBeforeClose" @click="discardAndClose">
            <X class="size-4" />
            <span>{{ t("viewer.discard") }}</span>
          </button>
          <button type="button" class="viewer-action-button viewer-action-button-primary" :disabled="isSavingBeforeClose" @click="saveAndClose">
            <Save class="size-4" />
            <span>{{ isSavingBeforeClose ? t("common.saving") : t("viewer.saveAndClose") }}</span>
          </button>
        </div>
      </section>
    </div>
  </main>
</template>
