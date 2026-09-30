import { convertFileSrc } from "@tauri-apps/api/core";
import { ref, watch, type WatchSource } from "vue";
import { ipasteApi } from "../lib/ipasteApi";

const MAX_THUMBNAIL_CACHE_SIZE = 120;
const successfulThumbnails = new Map<string, string>();
const pendingThumbnails = new Map<string, Promise<string>>();
const isTauri = "__TAURI_INTERNALS__" in window;

function readCachedThumbnail(path: string) {
  const thumbnailPath = successfulThumbnails.get(path);
  if (!thumbnailPath) return undefined;

  // Refresh the entry so frequently visible cards remain in the bounded cache.
  successfulThumbnails.delete(path);
  successfulThumbnails.set(path, thumbnailPath);
  return thumbnailPath;
}

function cacheThumbnail(path: string, thumbnailPath: string) {
  successfulThumbnails.delete(path);
  successfulThumbnails.set(path, thumbnailPath);

  if (successfulThumbnails.size <= MAX_THUMBNAIL_CACHE_SIZE) return;
  const oldestPath = successfulThumbnails.keys().next().value;
  if (oldestPath !== undefined) successfulThumbnails.delete(oldestPath);
}

function loadThumbnail(path: string): Promise<string> {
  const cachedThumbnail = readCachedThumbnail(path);
  if (cachedThumbnail) return Promise.resolve(cachedThumbnail);

  const pendingThumbnail = pendingThumbnails.get(path);
  if (pendingThumbnail) return pendingThumbnail;

  const request = ipasteApi.imageThumbnail(path)
    .then((thumbnailPath) => {
      if (!thumbnailPath) throw new Error("Image thumbnail command returned an empty path");
      cacheThumbnail(path, thumbnailPath);
      return thumbnailPath;
    })
    .finally(() => {
      pendingThumbnails.delete(path);
    });
  pendingThumbnails.set(path, request);
  return request;
}

function thumbnailSrc(path: string) {
  // convertFileSrc is only available in a Tauri webview. The browser fallback
  // intentionally returns the original source path unchanged.
  return isTauri ? convertFileSrc(path) : path;
}

/**
 * Keeps the card's original image visible while a local thumbnail is prepared.
 * Stale requests may fill the shared cache, but can never update a recycled card.
 */
export function useImageThumbnail(path: WatchSource<string | null>) {
  const source = ref("");

  watch(path, async (imagePath, _previousPath, onCleanup) => {
    let active = true;
    onCleanup(() => {
      active = false;
    });

    source.value = "";
    // Image data URLs already contain their bytes. Never send that payload through
    // the thumbnail command; persisted image clips supply a local filesystem path.
    if (!imagePath || imagePath.startsWith("data:")) return;

    try {
      const thumbnailPath = await loadThumbnail(imagePath);
      if (active) source.value = thumbnailSrc(thumbnailPath);
    } catch {
      // Keep the original image source and allow a later card to retry.
    }
  }, { immediate: true });

  return source;
}
