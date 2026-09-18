import assert from "node:assert/strict";
import { test } from "node:test";
import { createPinia, setActivePinia } from "pinia";
import { useIpasteStore } from "../src/stores/ipasteStore";
import { categoryOrderByIds, compareClipOrder } from "../src/lib/clipOrder";
import type { CategoryItem, ClipItem } from "../src/types";

const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
const clip = (id: string, isPinned = false, time = "2026-09-18T10:00:00Z"): ClipItem => ({
  id, isPinned, clipType: "text", contentHash: id, text: id, previewText: id,
  lastCapturedAt: time, favoriteCount: 0,
});
const saved = (id: string, sortOrder: number, isPinned = false): CategoryItem => ({
  id, isPinned, sortOrder, categoryId: "dev", clipSnapshotId: id, clipType: "text",
  contentHash: id, text: id, previewText: id, createdAt: "2026-09-18T10:00:00Z",
  updatedAt: "2026-09-18T10:00:00Z", syncState: "local",
});
const page = (clips: ClipItem[], hasMore = false) => ({ clips, hasMore, totalCount: 200, allCount: 200 });
function fixture() {
  setActivePinia(createPinia());
  const state = globalThis.__searchTest;
  state.requests = [];
  state.pinRequests = [];
  state.reorderRequests = [];
  state.listeners.clear();
  state.syncCount = 0;
  return { store: useIpasteStore(), ...state };
}

test("pinning ignores an old page, keeps selection and prevents duplicate submissions", async () => {
  const { store, requests, pinRequests } = fixture();
  const target = clip("older", false, "2026-09-10T10:00:00Z");
  store.clips = [clip("recent"), target];
  store.hasMoreClips = true;
  store.setSelectedIndex(1);
  const oldPage = store.loadMoreClips();
  const pending = store.togglePinned(store.selectedItem!);
  await store.togglePinned(store.selectedItem!);
  assert.equal(pinRequests.length, 1);
  assert.equal(store.isUpdatingPin, true);
  assert.equal(store.selectedItem?.isPinned, false);
  pinRequests[0].resolve({ ...target, isPinned: true });
  await flush();
  assert.deepEqual(pinRequests[0].args, ["older", "history", true]);
  assert.equal(store.selectedItem?.id, "older");
  requests[0].resolve(page([clip("stale")]));
  await oldPage;
  requests[1].resolve(page([{ ...target, isPinned: true }, clip("recent")]));
  await pending;
  assert.deepEqual(store.visibleItems.map((item) => item.id), ["older", "recent"]);
  assert.equal(store.selectedItem?.id, "older");
  assert.equal(store.isUpdatingPin, false);
});

test("unpinning an old record fills the intervening pages before restoring selection", async () => {
  const { store, requests, pinRequests } = fixture();
  const target = clip("old-pin", true, "2026-01-01T00:00:00Z");
  const normal = Array.from({ length: 130 }, (_, index) => clip(`normal-${String(index).padStart(3, "0")}`));
  store.clips = [target, ...normal.slice(0, 19)];
  store.hasMoreClips = true;
  const pending = store.togglePinned(store.selectedItem!);
  pinRequests[0].resolve({ ...target, isPinned: false });
  await flush();
  requests[0].resolve(page(normal.slice(0, 20), true));
  await flush();
  assert.deepEqual(requests[1].args, [20, 100, ""]);
  requests[1].resolve(page(normal.slice(20, 120), true));
  await flush();
  requests[2].resolve(page([...normal.slice(120), { ...target, isPinned: false }]));
  await pending;
  assert.equal(store.clips.length, 131);
  assert.equal(store.selectedItem?.id, target.id);
  assert.equal(store.selectedIndex, 130);
});

test("a rejected pin leaves data, ordering and selection unchanged", async () => {
  const { store, pinRequests, requests } = fixture();
  store.clips = [clip("recent"), clip("older", false, "2026-01-01T00:00:00Z")];
  store.setSelectedIndex(1);
  const before = JSON.stringify(store.clips);
  const pending = store.togglePinned(store.selectedItem!);
  pinRequests[0].reject(new Error("write failed"));
  await pending;
  assert.equal(JSON.stringify(store.clips), before);
  assert.equal(store.selectedItem?.id, "older");
  assert.match(store.error!, /write failed/);
  assert.equal(store.isUpdatingPin, false);
  assert.equal(requests.length, 0);
});

test("captures remain below pins without gaps after a partially loaded pinned group", async () => {
  const { store, listeners } = fixture();
  await store.bindEvents();
  const capture = (value: ClipItem) => listeners.get("ipaste://clipboard-captured")!({
    payload: { clip: value, clipTotalCount: 80, wasInserted: true },
  });
  const pin = clip("pin", true, "2026-01-01T00:00:00Z");
  store.clips = [pin, clip("ordinary")];
  store.hasMoreClips = true;
  store.setSelectedIndex(1);
  capture(clip("new", false, "2026-09-19T00:00:00Z"));
  assert.deepEqual(store.visibleItems.map((item) => item.id), ["pin", "new", "ordinary"]);
  assert.equal(store.selectedItem?.id, "ordinary");
  store.clips = [pin];
  store.setSelectedIndex(0);
  capture(clip("newer", false, "2026-09-20T00:00:00Z"));
  assert.deepEqual(store.clips.map((item) => item.id), ["pin"]);
  capture({ ...pin, lastCapturedAt: "2026-09-21T00:00:00Z" });
  assert.equal(store.selectedItem?.isPinned, true);
});

test("a capture during pagination causes a fresh contiguous read", async () => {
  const { store, requests, listeners } = fixture();
  await store.bindEvents();
  store.clips = [clip("pin", true), clip("ordinary")];
  store.hasMoreClips = true;
  const more = store.loadMoreClips();
  const fresh = clip("fresh", false, "2026-09-19T00:00:00Z");
  listeners.get("ipaste://clipboard-captured")!({ payload: { clip: fresh, clipTotalCount: 3, wasInserted: true } });
  requests[0].resolve(page([clip("stale")], true));
  await flush();
  assert.equal(requests[1].args[0], 0);
  requests[1].resolve(page([clip("pin", true), fresh, clip("ordinary")]));
  await more;
  assert.deepEqual(store.clips.map((item) => item.id), ["pin", "fresh", "ordinary"]);
});

test("captures during a reload restart the read instead of being overwritten", async () => {
  const { store, requests, listeners } = fixture();
  await store.bindEvents();
  store.clips = [clip("old")];
  const reload = store.reloadClips();
  const fresh = clip("fresh", true);
  listeners.get("ipaste://clipboard-captured")!({ payload: { clip: fresh, clipTotalCount: 2, wasInserted: true } });
  requests[0].resolve(page([clip("old")]));
  await flush();
  assert.equal(requests.length, 2);
  requests[1].resolve(page([fresh, clip("old")]));
  await reload;
  assert.deepEqual(store.clips.map((item) => item.id), ["fresh", "old"]);
});

test("search changes cancel pin refresh without replacing newer results", async () => {
  const { store, requests, pinRequests } = fixture();
  store.clips = [clip("target")];
  const pending = store.togglePinned(store.selectedItem!);
  pinRequests[0].resolve(clip("target", true));
  await flush();
  store.search = "match";
  const reload = store.reloadClips();
  requests[1].resolve(page([clip("match")]));
  await reload;
  requests[0].resolve(page([clip("target", true)]));
  await pending;
  assert.deepEqual(store.visibleItems.map((item) => item.id), ["match"]);
});

test("category pins are independent and a late response does not steal another category's selection", async () => {
  const { store, pinRequests } = fixture();
  store.clips = [clip("target")];
  store.categoryItems = [saved("first", 0), saved("target", 1), { ...saved("other", 0), categoryId: "other" }];
  store.selectedCategoryId = "dev";
  store.setSelectedIndex(1);
  const pending = store.togglePinned(store.selectedItem!);
  store.selectedCategoryId = "other";
  store.setSelectedIndex(0);
  pinRequests[0].resolve({ ...saved("target", 1), isPinned: true });
  await pending;
  assert.equal(store.selectedItem?.id, "other");
  assert.equal(store.clips[0].isPinned, false);
  store.selectedCategoryId = "dev";
  assert.deepEqual(store.visibleItems.map((item) => item.id), ["target", "first"]);
  assert.equal(store.categoryItems.find((item) => item.id === "target")?.sortOrder, 1);
});

test("group reordering preserves unpin positions and rejects cross-group order", () => {
  const items = [saved("first", 0), saved("pin", 1, true), saved("last", 2)];
  assert.equal(categoryOrderByIds(items, ["last", "pin", "first"]), null);
  assert.equal(categoryOrderByIds(items, ["pin", "first", "first"]), null);
  const reordered = categoryOrderByIds(items, ["pin", "last", "first"])!;
  assert.deepEqual(reordered.map((item) => item.id), ["last", "pin", "first"]);
  assert.deepEqual(reordered.map((item) => item.sortOrder), [0, 1, 2]);
});

test("pinning waits for category reorder persistence to prevent stale responses", async () => {
  const { store, pinRequests, reorderRequests } = fixture();
  store.categoryItems = [saved("first", 0), saved("pin", 1, true), saved("last", 2)];
  store.selectedCategoryId = "dev";
  const pending = store.reorderCategoryItems("dev", ["pin", "last", "first"]);
  assert.equal(store.isReorderingCategoryItems, true);
  await store.togglePinned(store.selectedItem!);
  assert.equal(pinRequests.length, 0);
  reorderRequests[0].resolve([...store.categoryItems]);
  await pending;
  assert.equal(store.isReorderingCategoryItems, false);
  assert.equal(store.visibleItems[0].isPinned, true);
});

test("category pinning schedules cloud sync and history pinning stays local", async () => {
  const { store, pinRequests, requests } = fixture();
  const scheduled: Array<() => void> = [];
  const originalSetTimeout = window.setTimeout;
  window.setTimeout = ((callback: () => void) => { scheduled.push(callback); return 1; }) as typeof window.setTimeout;
  try {
    store.cloud.enabled = true;
    store.categoryItems = [saved("saved", 0)];
    store.selectedCategoryId = "dev";
    const savedPin = store.togglePinned(store.selectedItem!);
    pinRequests[0].resolve({ ...saved("saved", 0), isPinned: true });
    await savedPin;
    assert.equal(scheduled.length, 1);
    scheduled[0]();
    await flush();
    assert.equal(globalThis.__searchTest.syncCount, 1);
    store.selectedCategoryId = "history";
    store.clips = [clip("local")];
    const historyPin = store.togglePinned(store.selectedItem!);
    pinRequests[1].resolve(clip("local", true));
    await flush();
    requests[0].resolve(page([clip("local", true)]));
    await historyPin;
    assert.equal(scheduled.length, 1);
  } finally {
    window.setTimeout = originalSetTimeout;
  }
});

test("a failed post-pin refresh is retried before loading the next offset", async () => {
  const { store, pinRequests, requests } = fixture();
  store.clips = [clip("target")];
  store.hasMoreClips = true;
  const pending = store.togglePinned(store.selectedItem!);
  pinRequests[0].resolve(clip("target", true));
  await flush();
  requests[0].reject(new Error("read failed"));
  await pending;
  assert.equal(store.selectedItem?.isPinned, true);
  const more = store.loadMoreClips();
  assert.equal(requests[1].args[0], 0);
  requests[1].resolve(page([clip("target", true), clip("next")]));
  await more;
  assert.deepEqual(store.clips.map((item) => item.id), ["target", "next"]);
});

test("history ordering resolves timestamp ties deterministically and sorts before filtering", () => {
  const { store } = fixture();
  store.clips = [clip("match-z"), clip("other", true), clip("match-b", true), clip("match-a", true)];
  store.search = "match";
  assert.deepEqual(store.visibleItems.map((item) => item.id), ["match-a", "match-b", "match-z"]);
  const equalInstant = [clip("z", false, "2026-09-18T11:00:00+01:00"), clip("a")];
  assert.deepEqual(equalInstant.sort(compareClipOrder).map((item) => item.id), ["a", "z"]);
});

test("the fourth pin is first and copying an earlier pin does not move it", async () => {
  const { store, listeners } = fixture();
  await store.bindEvents();
  store.clips = [1, 2, 3, 4].map((rank) => ({
    ...clip(`pin-${rank}`, true, `2026-09-0${5 - rank}T00:00:00Z`), pinOrder: rank,
  }));
  assert.deepEqual(store.visibleItems.map((item) => item.id), ["pin-4", "pin-3", "pin-2", "pin-1"]);
  listeners.get("ipaste://clipboard-captured")!({ payload: {
    clip: { ...store.clips[0], lastCapturedAt: "2026-09-30T00:00:00Z" },
    clipTotalCount: 4, wasInserted: false,
  } });
  assert.deepEqual(store.visibleItems.map((item) => item.id), ["pin-4", "pin-3", "pin-2", "pin-1"]);
  store.search = "pin-2";
  assert.equal(store.visibleItems[0].pinOrder, 2);
});

test("category pin order is independent of normal order and survives pinned dragging", () => {
  const { store } = fixture();
  const items = [saved("a", 0), { ...saved("b", 1, true), pinOrder: 2 },
    { ...saved("c", 2, true), pinOrder: 3 }, saved("d", 3)];
  store.selectedCategoryId = "dev";
  store.categoryItems = items;
  assert.deepEqual(store.visibleItems.map((item) => item.id), ["c", "b", "a", "d"]);
  store.categoryItems = categoryOrderByIds(items, ["b", "c", "d", "a"])!;
  assert.deepEqual(store.visibleItems.map((item) => item.id), ["b", "c", "d", "a"]);
  assert.equal(store.categoryItems.find((item) => item.id === "b")!.sortOrder, 1);
  assert.equal(store.categoryItems.find((item) => item.id === "c")!.sortOrder, 2);
  store.categoryItems = store.categoryItems.map((item) => ({ ...item, isPinned: false, pinOrder: null }));
  assert.deepEqual(store.visibleItems.map((item) => item.id), ["d", "b", "c", "a"]);
});
