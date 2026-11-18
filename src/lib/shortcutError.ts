import { t } from "../i18n";
import { formatShortcut } from "./format";

export function getShortcutErrorMessage(error: unknown, shortcut?: string): string {
  const message = error instanceof Error ? error.message : String(error);
  // Native shortcut errors currently arrive as strings. Never render their
  // debug payload (modifier flags, key codes, IDs) in the settings UI.
  if (message.includes("HotKey already registered:")) {
    return t("settings.shortcuts.conflict", {
      shortcut: shortcut ? formatShortcut(shortcut) : t("settings.tabs.shortcuts"),
    });
  }
  if (message.includes("主面板快捷键与应用中心快捷键不能相同")) {
    return t("settings.shortcuts.duplicate");
  }
  if (/快捷键格式无效|快捷键需要包含|请输入快捷键/.test(message)) {
    return t("settings.shortcuts.invalid");
  }
  return t("settings.shortcuts.updateFailed");
}
