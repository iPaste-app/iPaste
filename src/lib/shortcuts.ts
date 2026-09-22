// Match the platform-specific modifier represented by Tauri's default shortcuts.
export function shortcutsEqual(left: string, right: string, isMacOs: boolean) {
  const normalize = (shortcut: string) => shortcut.toLowerCase().split("+").map(token => {
    const modifier = token.trim();
    if (modifier === "commandorcontrol" || modifier === "cmdorctrl") return isMacOs ? "meta" : "control";
    if (["command", "cmd", "super"].includes(modifier)) return "meta";
    if (modifier === "ctrl") return "control";
    if (modifier === "option") return "alt";
    return modifier;
  }).sort().join("+");
  return normalize(left) === normalize(right);
}
