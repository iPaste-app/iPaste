import type { ClipItem } from "../types";

type SearchableClip = Pick<ClipItem, "clipType" | "displayName" | "previewText" | "text">;

export function matchesClipQuery(item: SearchableClip, query: string) {
  return !query || [
    item.displayName ?? "",
    item.previewText,
    item.clipType,
    item.clipType === "image" ? "图片 image" : item.text,
  ].some((value) => value.toLowerCase().includes(query));
}
