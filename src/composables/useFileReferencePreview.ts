import { convertFileSrc } from "@tauri-apps/api/core";
import {
  onScopeDispose,
  ref,
  watch,
  type Ref,
  type WatchSource,
} from "vue";
import { fileErrorMessage } from "../lib/fileError";
import { ipasteApi } from "../lib/ipasteApi";
import type { FileReferencePreview } from "../types";

const POLL_INTERVAL_MS = 15_000;
const refreshRevision = ref(0);
const pendingByPath = new Map<string, Promise<FileReferencePreview | null>>();
let subscriberCount = 0;
let pollTimer: number | null = null;
let surfaceVisible = true;

export type FileReferencePreviewState = {
  size: Ref<number | null>;
  thumbnailSrc: Ref<string>;
  dimensions: Ref<FileReferencePreview["dimensions"]>;
  error: Ref<string>;
  loading: Ref<boolean>;
};

/** Revalidate all visible file references, for example when the panel reopens. */
export function refreshFileReferencePreviews() {
  refreshRevision.value += 1;
}

/** Native panels can be hidden while the webview still reports document.hidden=false. */
export function setFileReferencePreviewSurfaceVisible(visible: boolean) {
  if (surfaceVisible === visible) return;
  surfaceVisible = visible;
  if (visible) refreshFileReferencePreviews();
}

/**
 * Loads native file metadata without ever exposing the original file as an image URL.
 * Requests for the same path are shared, while every focus/panel/poll refresh reaches
 * the backend so deleted or changed files cannot remain cached indefinitely.
 */
export function useFileReferencePreview(
  path: WatchSource<string | null>,
  itemRevision: WatchSource<string> = () => "",
): FileReferencePreviewState {
  const size = ref<number | null>(null);
  const thumbnailSrc = ref("");
  const dimensions = ref<FileReferencePreview["dimensions"]>(null);
  const error = ref("");
  const loading = ref(false);
  let activePath: string | null = null;
  let subscribed = false;

  watch(path, (currentPath) => {
    if (currentPath && !subscribed) {
      subscribed = true;
      subscribeToRefreshEvents();
    } else if (!currentPath && subscribed) {
      subscribed = false;
      unsubscribeFromRefreshEvents();
    }
  }, { immediate: true });
  onScopeDispose(() => {
    if (subscribed) unsubscribeFromRefreshEvents();
  });

  watch([path, itemRevision, refreshRevision], async ([currentPath], _previous, onCleanup) => {
    let active = true;
    onCleanup(() => { active = false; });

    if (currentPath !== activePath) {
      activePath = currentPath;
      size.value = null;
      thumbnailSrc.value = "";
      dimensions.value = null;
      error.value = "";
    }

    if (!currentPath) {
      loading.value = false;
      return;
    }

    loading.value = true;
    try {
      const preview = await loadFileReferencePreview(currentPath);
      if (!active || activePath !== currentPath) return;

      size.value = preview?.size ?? null;
      thumbnailSrc.value = preview?.thumbnailPath ? convertFileSrc(preview.thumbnailPath) : "";
      dimensions.value = preview?.dimensions ?? null;
      error.value = "";
    } catch (unknownError) {
      if (!active || activePath !== currentPath) return;

      size.value = null;
      thumbnailSrc.value = "";
      dimensions.value = null;
      error.value = fileErrorMessage(unknownError) ?? String(unknownError);
    } finally {
      if (active && activePath === currentPath) loading.value = false;
    }
  }, { immediate: true });

  return { size, thumbnailSrc, dimensions, error, loading };
}

function loadFileReferencePreview(path: string) {
  const pending = pendingByPath.get(path);
  if (pending) return pending;

  const request = ipasteApi.fileReferencePreview(path)
    .finally(() => {
      if (pendingByPath.get(path) === request) pendingByPath.delete(path);
    });
  pendingByPath.set(path, request);
  return request;
}

function subscribeToRefreshEvents() {
  subscriberCount += 1;
  if (subscriberCount !== 1) return;

  window.addEventListener("focus", handleWindowFocus);
  document.addEventListener("visibilitychange", handleVisibilityChange);
  pollTimer = window.setInterval(() => {
    if (surfaceVisible && !document.hidden) refreshFileReferencePreviews();
  }, POLL_INTERVAL_MS);
}

function unsubscribeFromRefreshEvents() {
  subscriberCount = Math.max(0, subscriberCount - 1);
  if (subscriberCount !== 0) return;

  window.removeEventListener("focus", handleWindowFocus);
  document.removeEventListener("visibilitychange", handleVisibilityChange);
  if (pollTimer !== null) window.clearInterval(pollTimer);
  pollTimer = null;
}

function handleVisibilityChange() {
  if (surfaceVisible && !document.hidden) refreshFileReferencePreviews();
}

function handleWindowFocus() {
  if (surfaceVisible) refreshFileReferencePreviews();
}
