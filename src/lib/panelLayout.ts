export function clipColumnCount(panelWidth: number, sidebarWidth = 0): 1 | 2 {
  return panelWidth - sidebarWidth < 320 ? 1 : 2;
}

export function selectionDelta(key: string, columns: number): number | null {
  const rowSize = Math.max(1, columns);
  switch (key) {
    case "ArrowDown": return rowSize;
    case "ArrowUp": return -rowSize;
    case "ArrowRight": return 1;
    case "ArrowLeft": return -1;
    default: return null;
  }
}

export function submenuPlacement(branch: { left: number; right: number }, width: number, viewportWidth: number) {
  const gap = 8;
  const fitsRight = branch.right + gap + width <= viewportWidth - gap;
  const fitsLeft = branch.left - gap - width >= gap;
  return { inline: !fitsRight && !fitsLeft, alignLeft: !fitsRight && fitsLeft };
}
