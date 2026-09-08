import assert from "node:assert/strict";
import { test } from "node:test";
import { createRenderer, h, markRaw, nextTick, reactive } from "vue";
import { createPinia, setActivePinia } from "pinia";
import ClipCard from "../src/components/ClipCard.vue";
import TopBar from "../src/components/TopBar.vue";
import { useIpasteStore } from "../src/stores/ipasteStore";
import { textStats } from "../src/lib/format";

function node(tag = "", text = "") {
  return markRaw({tag, text, props: {}, children: [], parent: null});
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
const clip = (id = "one", text = "hello") => ({
  id, text, collection: "history", clipType: "text", contentHash: id,
  previewText: text.slice(0, 180), lastCapturedAt: new Date().toISOString(),
  favoriteCount: 0, isPinned: false,
});
const page = (clips, hasMore = false) => ({clips, hasMore, totalCount: 100, allCount: 100});
function storeFixture() {
  setActivePinia(createPinia());
  globalThis.__searchTest.requests = [];
  return {store: useIpasteStore(), requests: globalThis.__searchTest.requests};
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

test("Chinese composition only publishes the committed search", async () => {
  const values = [];
  const props = reactive({modelValue: "", shortcut: "", settingsOpen: false,
    appendCopyEnabled: false, appendCopyTimeoutMinutes: 1,
    "onUpdate:modelValue": value => {values.push(value); props.modelValue = value;}});
  const root = node("root");
  const app = renderer.createApp({render: () => h(TopBar, props)});
  try {
    app.mount(root);
    const input = find(root, "input");
    input.props.onCompositionstart();
    input.props.onInput({target: {value: "zhong"}, isComposing: true});
    await nextTick();
    assert.deepEqual(values, []);
    input.props.onCompositionend({target: {value: "中"}});
    await nextTick();
    input.props.onInput({target: {value: "中"}, isComposing: false});
    assert.deepEqual(values, ["中"]);
    input.props.onInput({target: {value: "中文a"}});
    assert.deepEqual(values, ["中", "中文a"]);
  } finally {app.unmount();}
});

test("typing reuses matching card objects and still searches beyond the loaded page", async () => {
  const {store, requests} = storeFixture();
  store.clips = [clip("one", "hello needle"), clip("two", "other")];
  const visible = store.visibleItems[0];
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
