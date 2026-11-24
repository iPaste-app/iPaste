import assert from "node:assert/strict";
import { test } from "node:test";
import { createRenderer, h, markRaw, nextTick, reactive } from "vue";
import { createPinia, setActivePinia } from "pinia";
import ClipCard from "../src/components/ClipCard.vue";
import QuickSearchPanel from "../src/components/QuickSearchPanel.vue";
import TopBar from "../src/components/TopBar.vue";
import { useIpasteStore } from "../src/stores/ipasteStore";
import { textStats } from "../src/lib/format";

let focused;
function node(tag = "", text = "") {
  const listeners = new Map();
  return markRaw({tag, text, props: {}, children: [], parent: null,
    focus() { focused = this; },
    addEventListener(type, listener) {
      listeners.set(type, [...(listeners.get(type) ?? []), listener]);
    },
    dispatchEvent({type}) {
      const event = {type, target: this};
      this.props[`on${type[0].toUpperCase()}${type.slice(1)}`]?.(event);
      for (const listener of listeners.get(type) ?? []) listener(event);
    },
    scrollIntoView() {},
    querySelector(selector) {
      return selector === '[aria-selected="true"]' ? all(this, el => el.props["aria-selected"] === true)[0] : null;
    },
  });
}
const renderer = createRenderer({
  createElement: tag => node(tag), createText: text => node("#text", text),
  createComment: () => node("#comment"),
  setText: (el, text) => {el.text = text;},
  setElementText: (el, text) => {el.text = text; el.children = [];},
  patchProp: (el, key, _, value) => {el.props[key] = value;},
  insert(el, parent, anchor) {
    if (el.parent) el.parent.children.splice(el.parent.children.indexOf(el), 1);
    el.parent = parent;
    const index = anchor ? parent.children.indexOf(anchor) : -1;
    if (index < 0) parent.children.push(el); else parent.children.splice(index, 0, el);
  },
  remove(el) {if (el.parent) el.parent.children.splice(el.parent.children.indexOf(el), 1);},
  parentNode: el => el.parent,
  nextSibling: el => el.parent?.children[el.parent.children.indexOf(el) + 1] ?? null,
});
function find(root, tag) {
  if (root.tag === tag) return root;
  for (const child of root.children) {const found = find(child, tag); if (found) return found;}
}
function all(root, predicate) {
  return [...(predicate(root) ? [root] : []), ...root.children.flatMap(child => all(child, predicate))];
}
const clip = (id = "one", text = "hello") => ({
  id, text, collection: "history", clipType: "text", contentHash: id,
  previewText: text.slice(0, 180), lastCapturedAt: new Date().toISOString(),
  favoriteCount: 0, isPinned: false,
});
const page = (clips, hasMore = false) => ({clips, hasMore, totalCount: 100, allCount: 100});
function storeFixture() {
  const pinia = createPinia();
  setActivePinia(pinia);
  globalThis.__searchTest.requests = [];
  return {store: useIpasteStore(pinia), requests: globalThis.__searchTest.requests, pinia};
}

const flush = async () => {
  await new Promise(resolve => setImmediate(resolve));
  await nextTick();
};

function panelFixture(appCenter = false, seed = () => {}) {
  const {store, requests, pinia} = storeFixture();
  seed(store);
  const root = node("root");
  const state = reactive({open: true, filter: "", dismissals: 0, reveals: 0});
  const timers = new Map();
  const {setTimeout: originalSet, clearTimeout: originalClear} = window;
  let timerId = 0;
  window.setTimeout = callback => { timers.set(++timerId, callback); return timerId; };
  window.clearTimeout = id => { timers.delete(id); };
  let panel;
  const app = renderer.createApp({render: () => state.open ? h(QuickSearchPanel, {
    ref: value => { panel = value; },
    appCenter,
    appFilter: state.filter,
    "onUpdate:appFilter": value => { state.filter = value; },
    onDismiss: () => { state.dismissals++; state.open = false; },
    onReveal: () => { state.reveals++; state.open = false; },
  }) : null});
  app.use(pinia);
  app.mount(root);
  const input = find(root, "input");
  return {
    root, state, store, requests, input, panel,
    options: () => all(root, el => el.props.role === "option"),
    status: () => all(root, el => el.props.role === "status")[0]?.text,
    type(value: string) { input.value = value; input.dispatchEvent({type: "input"}); },
    runDebounce() {
      for (const [id, callback] of timers) { timers.delete(id); callback(); }
    },
    unmount() {
      app.unmount();
      window.setTimeout = originalSet;
      window.clearTimeout = originalClear;
    },
  };
}

test("long cards bound DOM text, preserve copy content and reuse unchanged character metrics", async () => {
  const fullText = "测试 abc ".repeat(61029);
  let segmentCalls = 0;
  const NativeSegmenter = Intl.Segmenter;
  Intl.Segmenter = class extends NativeSegmenter {
    segment(text) {segmentCalls++; return super.segment(text);}
  };
  let applied;
  const props = reactive({item: clip("large", fullText), index: 0, selected: false,
    categoryTags: [], editingName: null, reorderEnabled: false, onApply: item => {applied = item;}});
  const root = node("root");
  const app = renderer.createApp({render: () => h(ClipCard, props)});
  try {
    app.mount(root);
    const preview = find(root, "p").text;
    assert.ok(preview.length <= 500);
    assert.ok(fullText.startsWith(preview));
    find(root, "article").props.onDblclick();
    assert.equal(applied.text, fullText);
    const initialCalls = segmentCalls;
    props.item = {...props.item};
    await nextTick();
    assert.equal(segmentCalls, initialCalls, "new result objects with unchanged text must not recount");
    props.item = {...props.item, text: "changed"};
    await nextTick();
    assert.equal(segmentCalls, initialCalls + 1);
    assert.equal(find(root, "p").text, "changed");
  } finally {app.unmount(); Intl.Segmenter = NativeSegmenter;}
});

test("character metrics preserve emoji and combining-character counts", () => {
  assert.equal(textStats("👨‍👩‍👧‍👦e\u0301中"), 'stats.chars:{"value":3}');
});

test("the search trigger stays outside the right action group, with update immediately before App Center", async () => {
  const root = node("root");
  const props = reactive({
    searchOpen: false, settingsOpen: false, appCenterOpen: false,
    appendCopyEnabled: false, appendCopyTimeoutMinutes: 1, hasUpdate: false,
    onToggleSearch: () => { props.searchOpen = !props.searchOpen; },
  });
  const app = renderer.createApp({render: () => h(TopBar, props)});
  try {
    app.mount(root);
    const header = find(root, "header");
    const trigger = find(header, "button");
    const actions = header.children.find(el => el.props.class === "top-bar-actions");
    const actionLabels = () => actions.children.filter(el => el.tag === "button").map(el => el.props["aria-label"]);
    assert.equal(find(header, "input"), undefined);
    assert.equal(trigger.parent, header);
    assert.deepEqual(trigger.children.map(child => child.tag), ["svg", "span"]);
    assert.equal(find(trigger, "span").text, "shortcut.search");
    assert.equal(trigger.props["aria-label"], "shortcut.search");
    assert.equal(trigger.props["data-tooltip"], undefined, "returning focus should not show a duplicate label");
    assert.equal(trigger.props["aria-expanded"], false);
    assert.equal(actionLabels()[0], "appCenter.toggle");

    trigger.props.onClick({stopPropagation() {}});
    props.hasUpdate = true;
    await nextTick();
    assert.equal(trigger.props["aria-expanded"], true);
    assert.equal(trigger.props["aria-controls"], "quick-search-panel");
    assert.deepEqual(actionLabels().slice(0, 2), ["topBar.openUpdate", "appCenter.toggle"]);

    trigger.props.onClick({stopPropagation() {}});
    await nextTick();
    assert.equal(trigger.props["aria-expanded"], false);
  } finally {app.unmount();}
});

test("Chinese composition only publishes the committed search", async () => {
  const f = panelFixture();
  try {
    assert.equal(focused, f.input);
    f.type("previous");
    f.runDebounce();
    f.input.dispatchEvent({type: "compositionstart"});
    f.type("zhong");
    assert.equal(f.panel.handleKey("Enter"), false);
    assert.equal(f.panel.handleKey("Escape"), false);
    f.requests[0].resolve(page([clip("previous-result")]));
    await flush();
    assert.equal(f.input.value, "zhong");
    f.runDebounce();
    assert.equal(f.requests.length, 1);

    f.input.value = "中";
    f.input.dispatchEvent({type: "compositionend"});
    f.type("中");
    await nextTick();
    f.runDebounce();
    assert.equal(f.requests.length, 2);
    assert.deepEqual(f.requests[1].args, [0, 20, "中"]);
    assert.equal(f.state.reveals, 0);
    assert.equal(f.state.dismissals, 0);
  } finally {f.unmount();}
});

test("category results use their own keyboard selection and reveal the original item", async () => {
  const f = panelFixture(false, store => {
    store.selectedCategoryId = "saved";
    store.categoryItems = ["unmatched", "topic A", "topic B"].map((value, sortOrder) => ({
      ...clip(String(sortOrder), value), categoryId: "saved", clipSnapshotId: String(sortOrder),
      createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:00:00Z", sortOrder,
    }));
  });
  try {
    f.type("TOPIC");
    await nextTick();
    assert.equal(f.options().length, 2);
    assert.equal(f.store.visibleItems.length, 3);
    assert.equal(f.store.selectedIndex, 0);
    f.panel.handleKey("ArrowDown");
    await nextTick();
    assert.equal(f.options()[1].props["aria-selected"], true);
    assert.equal(f.input.props["aria-activedescendant"], f.options()[1].props.id);
    assert.equal(f.store.selectedIndex, 0);
    assert.equal(f.panel.handleKey("Enter", true), true);
    assert.equal(f.state.reveals, 0);
    f.panel.handleKey("Enter");
    await flush();
    assert.equal(f.state.reveals, 1);
    assert.equal(f.store.selectedItem?.id, "2");
    assert.equal(f.store.error, null);
    assert.equal(f.requests.length, 0);
  } finally {f.unmount();}
});

test("history queries discard stale results and reveal matches beyond the loaded prefix", async () => {
  const recent = clip("000", "recent only");
  const match = {...clip("100", "target match"), lastCapturedAt: "2026-01-01T00:00:00Z"};
  const f = panelFixture(false, store => { store.clips = [recent]; });
  try {
    f.type("old");
    f.runDebounce();
    f.type("target");
    f.requests[0].resolve(page([clip("old")]));
    await flush();
    assert.equal(f.options().length, 0);
    f.runDebounce();
    assert.deepEqual(f.requests[1].args, [0, 20, "target"]);
    f.requests[1].resolve(page([match]));
    await flush();
    assert.equal(f.options().length, 1);
    assert.deepEqual(f.store.visibleItems.map(item => item.id), [recent.id]);
    assert.equal(f.store.search, "");

    f.panel.handleKey("Enter");
    assert.deepEqual(f.requests[2].args, [0, 100, ""]);
    const prefix = [recent, ...Array.from({length: 99}, (_, i) => clip(String(i + 1).padStart(3, "0")))];
    f.requests[2].resolve(page(prefix, true));
    await flush();
    assert.deepEqual(f.requests[3].args, [100, 100, ""]);
    f.requests[3].resolve(page([match], true));
    await flush();
    assert.equal(f.state.reveals, 1);
    assert.equal(f.store.visibleItems.length, 101);
    assert.equal(f.store.selectedItem?.id, match.id);
    const more = f.store.loadMoreClips();
    assert.deepEqual(f.requests[4].args, [101, 20, ""]);
    f.requests[4].resolve(page([]));
    await more;
  } finally {f.unmount();}
});

test("dismissing during a history reveal prevents a late reply from changing the list", async () => {
  const f = panelFixture(false, store => { store.clips = [clip("existing")]; });
  try {
    f.type("hidden");
    f.runDebounce();
    const match = clip("hidden");
    f.requests[0].resolve(page([match]));
    await flush();
    f.panel.handleKey("Enter");
    f.panel.handleKey("Escape");
    await nextTick();
    f.requests[1].resolve(page([match]));
    await flush();
    assert.equal(f.state.dismissals, 1);
    assert.equal(f.state.reveals, 0);
    assert.deepEqual(f.store.visibleItems.map(item => item.id), ["existing"]);
  } finally {f.unmount();}
});

test("scrolling the search results fetches another matching page, not another browse page", async () => {
  const f = panelFixture(false, store => { store.clips = [clip("existing")]; });
  try {
    f.type("match");
    f.runDebounce();
    f.requests[0].resolve(page(Array.from({length: 20}, (_, i) => clip(`match-${i}`)), true));
    await flush();
    const viewport = all(f.root, el => el.props.class === "quick-search-results")[0];
    Object.assign(viewport, {scrollHeight: 700, scrollTop: 350, clientHeight: 350});
    viewport.props.onScroll();
    viewport.props.onScroll();
    assert.equal(f.requests.length, 2);
    assert.deepEqual(f.requests[1].args, [20, 20, "match"]);
    f.requests[1].resolve(page([clip("match-20")]));
    await flush();
    assert.equal(f.options().length, 21);
    assert.equal(f.options()[0].props["aria-selected"], true);
    assert.deepEqual(f.store.visibleItems.map(item => item.id), ["existing"]);
  } finally {f.unmount();}
});

test("a vanished result leaves the panel open and explains why it cannot be located", async () => {
  const f = panelFixture();
  try {
    f.type("removed");
    f.runDebounce();
    f.requests[0].resolve(page([clip("removed")]));
    await flush();
    f.panel.handleKey("Enter");
    f.requests[1].resolve(page([]));
    await flush();
    assert.equal(f.state.open, true);
    assert.equal(f.status(), "quickSearch.revealError");
  } finally {f.unmount();}
});

test("App Center retains its own filter without starting a clip search", async () => {
  const f = panelFixture(true);
  try {
    f.type("mfa");
    await nextTick();
    f.runDebounce();
    assert.equal(f.state.filter, "mfa");
    assert.equal(f.requests.length, 0);
    assert.equal(f.options().length, 0);
    assert.equal(f.panel.handleKey("Enter"), false);
    assert.equal(f.panel.handleKey("Escape"), true);
    assert.equal(f.state.dismissals, 1);
  } finally {f.unmount();}
});

test("typing reuses matching card objects and still searches beyond the loaded page", async () => {
  const {store, requests} = storeFixture();
  store.clips = [clip("one", "hello needle"), clip("two", "other")];
  const visible = store.visibleItems.find(item => item.id === "one");
  store.search = "needle";
  assert.equal(store.visibleItems.length, 1);
  assert.equal(store.visibleItems[0], visible);
  const reload = store.reloadClips();
  assert.deepEqual(requests[0].args, [0, 20, "needle"]);
  requests[0].resolve(page([clip("match", "long body needle")]));
  await reload;
  assert.equal(store.visibleItems[0].id, "match");
});

test("old results are ignored even before the next debounced request starts", async () => {
  const {store, requests} = storeFixture();
  store.clips = [clip()];
  store.search = "old";
  const reload = store.reloadClips();
  store.search = "new";
  requests[0].resolve(page([clip("stale")]));
  await reload;
  assert.equal(store.clips[0].id, "one");
});

test("pagination cannot append stale results or mix offsets across searches", async () => {
  const {store, requests} = storeFixture();
  store.clips = [clip()];
  store.hasMoreClips = true;
  const more = store.loadMoreClips();
  store.search = "new";
  const reload = store.reloadClips();
  requests[1].resolve(page([clip("new-match")], true));
  await reload;
  requests[0].resolve(page([clip("old-page")]));
  await more;
  assert.deepEqual(store.clips.map(item => item.id), ["new-match"]);
  assert.equal(store.isLoadingMoreClips, false);
  store.search = "newer";
  await store.loadMoreClips();
  assert.equal(requests.length, 2);
});
