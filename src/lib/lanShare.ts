import { invoke } from "@tauri-apps/api/core";
import { t, type I18nKey } from "../i18n";
import type { ClipViewItem } from "../types";

export type LanShare = {
  id: string;
  sourceKey: string;
  url: string;
  qrSvg: string;
  expiresAt: number;
  replaced: boolean;
};

const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const lanShareApi = {
  start(item: ClipViewItem): Promise<LanShare> {
    if (!isTauri) return Promise.reject("SHARE_NATIVE_ONLY");
    return invoke("start_lan_share", { id: item.id, collection: item.collection });
  },
  current(): Promise<LanShare | null> {
    return isTauri ? invoke("get_lan_share") : Promise.resolve(null);
  },
  stop(id: string): Promise<void> {
    return isTauri ? invoke("stop_lan_share", { id }) : Promise.resolve();
  },
};

export function shareError(error: unknown): string {
  const messages: Record<string, I18nKey> = {
    SHARE_NATIVE_ONLY: "share.error.nativeOnly",
    SHARE_NO_NETWORK: "share.error.network",
    SHARE_BIND_FAILED: "share.error.start",
    SHARE_NOT_FOUND: "share.error.missing",
    SHARE_READ_FAILED: "share.error.read",
    FILE_NOT_FOUND: "share.error.read",
    FILE_ACCESS_DENIED: "share.error.read",
    FILE_NOT_REGULAR: "share.error.read",
    FILE_UNAVAILABLE: "share.error.read",
  };
  return t(messages[String(error)] ?? "share.error.start");
}

export function shareCountdown(expiresAt: number, now: number): string {
  const seconds = Math.max(0, Math.ceil((expiresAt - now) / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}
