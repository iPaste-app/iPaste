import { convertFileSrc } from "@tauri-apps/api/core";
import type { ClipItem, ClipViewItem } from "../types";

const isTauri = "__TAURI_INTERNALS__" in window;

export function clipImageSrc(item: Pick<ClipItem | ClipViewItem, "clipType" | "text">) {
  if (item.clipType !== "image") return "";
  if (!isTauri || item.text.startsWith("data:")) return item.text;
  return convertFileSrc(item.text);
}
