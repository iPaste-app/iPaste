import type { ClipType } from "../types";

const relativeFormatter = new Intl.RelativeTimeFormat("zh-CN", { numeric: "auto" });

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

  if (Math.abs(minutes) < 1) return "刚刚";
  if (Math.abs(minutes) < 60) return relativeFormatter.format(minutes, "minute");

  const hours = Math.round(minutes / 60);
  if (Math.abs(hours) < 24) return relativeFormatter.format(hours, "hour");

  const days = Math.round(hours / 24);
  if (Math.abs(days) < 7) return relativeFormatter.format(days, "day");

  return new Intl.DateTimeFormat("zh-CN", {
    month: "short",
    day: "numeric",
  }).format(new Date(value));
}

export function typeLabel(type: ClipType) {
  const labels: Record<ClipType, string> = {
    text: "文本",
    link: "链接",
    color: "颜色",
    image: "图片",
    file: "文件",
    html: "HTML",
  };

  return labels[type] ?? "文本";
}

export function textStats(text: string) {
  const chars = text.length;
  const words = text.trim() ? text.trim().split(/\s+/).length : 0;
  const hasCjk = /[\u4E00-\u9FFF]/.test(text);

  if (chars > 999) return `${(chars / 1000).toFixed(1)} 千字符`;
  if (!hasCjk && words > 1 && /[A-Za-z0-9]/.test(text)) return `${words} 词`;
  return `${chars} 字符`;
}

export function syncStateLabel(value: string) {
  const labels: Record<string, string> = {
    local: "本地",
    syncing: "同步中",
    synced: "已同步",
    conflict: "冲突",
  };

  return labels[value] ?? "本地";
}

export function categoryDisplayName(name: string) {
  const labels: Record<string, string> = {
    "Dev Snippets": "开发片段",
  };

  return labels[name] ?? name;
}

function platformModifier() {
  const platform = navigator.platform.toLowerCase();
  if (platform.includes("mac")) return "Cmd";
  return "Ctrl";
}
