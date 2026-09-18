import type { CategoryItem, ClipItem } from "../types";

function compareId(left: string, right: string) {
  return left < right ? -1 : left > right ? 1 : 0;
}

export function compareClipOrder(left: ClipItem, right: ClipItem) {
  return Number(right.isPinned) - Number(left.isPinned)
    || (left.isPinned ? (right.pinOrder ?? Number.MIN_SAFE_INTEGER) - (left.pinOrder ?? Number.MIN_SAFE_INTEGER) : 0)
    || Date.parse(right.lastCapturedAt) - Date.parse(left.lastCapturedAt)
    || compareId(left.id, right.id);
}

export function compareCategoryItemOrder(left: CategoryItem, right: CategoryItem) {
  return compareId(left.categoryId, right.categoryId)
    || Number(right.isPinned) - Number(left.isPinned)
    || (left.isPinned ? (right.pinOrder ?? Number.MIN_SAFE_INTEGER) - (left.pinOrder ?? Number.MIN_SAFE_INTEGER) : 0)
    || left.sortOrder - right.sortOrder
    || Date.parse(right.createdAt) - Date.parse(left.createdAt)
    || compareId(left.id, right.id);
}

// Pinned order is independent of the normal order. Keep pinned items in their
// original slots so dragging a pin does not change its eventual unpin position.
export function categoryOrderByIds(items: CategoryItem[], ids: string[]) {
  const byId = new Map(items.map((item) => [item.id, item]));
  if (ids.length !== items.length || new Set(ids).size !== ids.length) return null;
  const ordered = ids.map((id) => byId.get(id));
  if (ordered.some((item) => !item)) return null;
  const next = ordered as CategoryItem[];
  if (next.some((item, index) => item.isPinned && index > 0 && !next[index - 1].isPinned)) return null;

  const baseline = [...items].sort((left, right) =>
    left.sortOrder - right.sortOrder
      || Date.parse(right.createdAt) - Date.parse(left.createdAt)
      || compareId(left.id, right.id),
  );
  const pinned = next.filter((item) => item.isPinned);
  const ordinary = next.filter((item) => !item.isPinned);
  const pinRanks = new Map(pinned.map((item, index) => [item.id, pinned.length - index]));
  let ordinaryIndex = 0;
  return baseline.map((slot, sortOrder) => ({
    ...(slot.isPinned ? slot : ordinary[ordinaryIndex++]),
    sortOrder,
    pinOrder: slot.isPinned ? pinRanks.get(slot.id)! : null,
  }));
}
