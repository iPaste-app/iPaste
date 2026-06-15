import type { ClipType } from "../types";
import { currentLocale, t } from "../i18n";

export function formatShortcut(shortcut: string) {
  return shortcut
    .replace("CommandOrControl", platformModifier())
    .split("+")
    .join(" + ");
}

export function formatTime(value: string) {
  const timestamp = new Date(value).getTime();
  const delta = timestamp - Date.now();
  const minutes = Math.round(delta / 60_000);
  const locale = currentLocale.value;
  const relativeFormatter = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });

  if (Math.abs(minutes) < 1) return t("time.justNow");
  if (Math.abs(minutes) < 60) return relativeFormatter.format(minutes, "minute");

  const hours = Math.round(minutes / 60);
  if (Math.abs(hours) < 24) return relativeFormatter.format(hours, "hour");

  const days = Math.round(hours / 24);
  if (Math.abs(days) < 7) return relativeFormatter.format(days, "day");

  return new Intl.DateTimeFormat(locale, {
    month: "short",
    day: "numeric",
  }).format(new Date(value));
}

export function typeLabel(type: ClipType) {
  const labels: Record<ClipType, ReturnType<typeof t>> = {
    text: t("type.text"),
    link: t("type.link"),
    color: t("type.color"),
    image: t("type.image"),
    file: t("type.file"),
    html: t("type.html"),
  };

  return labels[type] ?? t("type.text");
}

export function textStats(text: string) {
  const chars = text.length;
  const words = text.trim() ? text.trim().split(/\s+/).length : 0;
  const hasCjk = /[\u4E00-\u9FFF]/.test(text);

  if (chars > 999) return t("stats.kChars", { value: (chars / 1000).toFixed(1) });
  if (!hasCjk && words > 1 && /[A-Za-z0-9]/.test(text)) return t("stats.words", { value: words });
  return t("stats.chars", { value: chars });
}

export function syncStateLabel(value: string) {
  const labels: Record<string, string> = {
    local: t("sync.local"),
    syncing: t("sync.syncing"),
    synced: t("sync.synced"),
    conflict: t("sync.conflict"),
  };

  return labels[value] ?? t("sync.local");
}

export function categoryDisplayName(name: string) {
  const labels: Record<string, string> = {
    "Dev Snippets": t("category.devSnippets"),
    "开发片段": t("category.devSnippets"),
  };

  return labels[name] ?? name;
}

function platformModifier() {
  const platform = navigator.platform.toLowerCase();
  if (platform.includes("mac")) return "Cmd";
  return "Ctrl";
}
