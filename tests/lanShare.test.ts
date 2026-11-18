import assert from "node:assert/strict";
import { test } from "node:test";
import { createRenderer, h, nextTick } from "vue";
import { useLanShare } from "../src/composables/useLanShare";
import { shareCountdown, type LanShare } from "../src/lib/lanShare";
import { copySharedText, remainingTime } from "../src-tauri/src/lan_share/page.js";

const state = globalThis.__lanShareTest;
const item = { id: "clip-a", collection: "history", clipType: "text", text: "sample" } as const;
const info = (): LanShare => ({
  id: "random-token", sourceKey: "history:clip-a", url: "http://192.168.1.8:1234/s/random-token",
  qrSvg: '<svg xmlns="http://www.w3.org/2000/svg"></svg>', expiresAt: Date.now() + 300_000, replaced: false,
});
const flush = async () => { await new Promise((resolve) => setImmediate(resolve)); await nextTick(); };

function fixture() {
  state.calls = [];
  state.copied = [];
  state.handlers = {};
  const renderer = createRenderer({
    createElement: () => ({}), createText: () => ({}), createComment: () => ({}),
    insert() {}, remove() {}, setText() {}, setElementText() {}, patchProp() {},
    parentNode: () => null, nextSibling: () => null,
  });
  let share: ReturnType<typeof useLanShare>;
  const app = renderer.createApp({ setup() { share = useLanShare(item as any); return () => h("div"); } });
  return { mount() { app.mount({}); return share!; }, close() { app.unmount(); } };
}

test("closing the dialog keeps the native share alive and sends only the stored item identity", async () => {
  const view = fixture();
  const current = info();
  state.handlers.start_lan_share = async () => current;
  const share = view.mount();
  await flush();
  assert.equal(share.active.value, true);
  assert.deepEqual(state.calls[0], { command: "start_lan_share", args: { id: "clip-a", collection: "history" } });
  assert.equal(share.share.value?.expiresAt, current.expiresAt);
  view.close();
  assert.equal(state.calls.some(({ command }) => command === "stop_lan_share"), false);
});

test("copying uses the ephemeral clipboard and manual stop targets the exact session", async () => {
  const view = fixture();
  const current = info();
  state.handlers.start_lan_share = async () => current;
  const share = view.mount();
  await flush();
  await share.copyLink();
  assert.deepEqual(state.copied, [current.url]);
  assert.equal(share.copied.value, true);
  await share.stop();
  assert.equal(share.active.value, false);
  assert.deepEqual(state.calls.at(-1), { command: "stop_lan_share", args: { id: current.id } });
  view.close();
});

test("failed sharing presents an error and never advertises a fake link", async () => {
  const view = fixture();
  state.handlers.start_lan_share = async () => { throw "SHARE_NO_NETWORK"; };
  const share = view.mount();
  await flush();
  assert.equal(share.active.value, false);
  assert.equal(share.qrSource.value, "");
  assert.equal(share.error.value, "share.error.network");
  view.close();
});

test("closing during startup does not mutate a disposed dialog or cancel the five-minute share", async () => {
  const view = fixture();
  let resolve;
  state.handlers.start_lan_share = () => new Promise((done) => { resolve = done; });
  const share = view.mount();
  view.close();
  resolve(info());
  await flush();
  assert.equal(share.share.value, null);
  assert.equal(state.calls.some(({ command }) => command === "stop_lan_share"), false);
});

test("expired shares cannot be copied even before the next status request", async () => {
  const view = fixture();
  state.handlers.start_lan_share = async () => ({ ...info(), expiresAt: Date.now() - 1 });
  const share = view.mount();
  await flush();
  assert.equal(share.active.value, false);
  await share.copyLink();
  assert.deepEqual(state.copied, []);
  view.close();
});

test("countdown is clamped at zero and does not round a new share down to 4:59", () => {
  for (const format of [shareCountdown, remainingTime]) {
    assert.equal(format(300_001, 1), "5:00");
    assert.equal(format(300_001, 2), "5:00");
    assert.equal(format(300_001, 1001), "4:59");
    assert.equal(format(300_001, 301_000), "0:00");
  }
});

test("copy checks the current wall clock after sleep rather than a stale UI timer", async () => {
  const view = fixture();
  const current = info();
  state.handlers.start_lan_share = async () => current;
  const share = view.mount();
  await flush();
  const actualNow = Date.now;
  try {
    Date.now = () => current.expiresAt + 1;
    await share.copyLink();
    assert.deepEqual(state.copied, []);
    assert.equal(share.active.value, false);
  } finally {
    Date.now = actualNow;
    view.close();
  }
});

function selection() {
  const calls: unknown[] = [];
  return { calls, element: { focus: () => calls.push("focus"), select: () => calls.push("select"), setSelectionRange: (...range) => calls.push(range) } };
}

test("receiver copies exact Unicode and whitespace using the modern clipboard when available", async () => {
  const { element, calls } = selection();
  const text = "中文🙂\n\t  text\r\n";
  const writes: string[] = [];
  const success = await copySharedText(text, element, { clipboard: { writeText: async (value) => { writes.push(value); } }, execCommand: () => { throw Error("must not run"); } });
  assert.equal(success, true);
  assert.deepEqual(writes, [text]);
  assert.deepEqual(calls, []);
});

test("receiver falls back to selection-based copy on a plain HTTP page", async () => {
  const { element, calls } = selection();
  const success = await copySharedText("你好", element, { clipboard: undefined, execCommand: (command) => command === "copy" });
  assert.equal(success, true);
  assert.deepEqual(calls, ["focus", "select", [0, 2]]);
});

test("receiver never reports success when both clipboard methods fail", async () => {
  const { element, calls } = selection();
  const success = await copySharedText("selected", element, { clipboard: { writeText: async () => { throw Error("denied"); } }, execCommand: () => false });
  assert.equal(success, false);
  assert.deepEqual(calls, ["focus", "select", [0, 8]]);
});
