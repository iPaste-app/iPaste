import assert from "node:assert/strict";
import { test } from "node:test";
import { createRenderer, reactive } from "vue";
import { useOcrModels } from "../src/composables/useOcrModels";

// The runner replaces only desktop I/O; Vue's state and lifecycle run unchanged.
const state = globalThis.__ocrTest;
const total = { fast: 6_932_119, best: 31_824_456 };
const renderer = createRenderer({
  createComment: () => ({}), createText: () => ({}), createElement: () => ({}),
  insert() {}, remove() {}, setText() {}, setElementText() {}, patchProp() {},
  parentNode: () => null, nextSibling: () => null,
});

async function fixture() {
  const available = new Set(["fast"]);
  const damaged = new Set<string>();
  const calls = [];
  let listener;
  let rejectDownload = false;
  let releaseDownload;
  let unlistened = false;
  const store = reactive({
    ocrMode: "fast",
    async loadSettings() {},
    async updateOcrMode(mode) {
      if (damaged.has(mode)) throw new Error("Model files failed verification");
      store.ocrMode = mode;
    },
  });
  function status(mode) {
    const installed = available.has(mode) && !damaged.has(mode);
    const hasResources = available.size > 0 || damaged.size > 0;
    return {
      mode, installed, needsRepair: damaged.has(mode), hasResources, totalBytes: total[mode],
      // An unused model can share a valid classifier; a damaged model can have no valid files.
      downloadedBytes: installed ? total[mode] : damaged.has(mode) ? 0 : hasResources ? 500 : 0,
    };
  }
  state.store = store;
  state.listen = async (_, callback) => { listener = callback; return () => { unlistened = true; }; };
  state.api = {
    async ocrInstallStatus(mode) { return status(mode); },
    async installOcrAssets(mode, activate) {
      calls.push({ mode, activate });
      await new Promise<void>((resolve) => { releaseDownload = resolve; });
      if (rejectDownload) throw new Error("HTTP 403");
      available.add(mode);
      damaged.delete(mode);
      if (activate) store.ocrMode = mode;
      return status(mode);
    },
    async removeOcrAssets() { available.clear(); damaged.clear(); },
  };
  let model;
  const app = renderer.createApp({ setup() { model = useOcrModels(); return () => null; } });
  app.mount({});
  // Drain the async listener and initial status requests.
  await new Promise((resolve) => setImmediate(resolve));
  return {
    model, store, calls, available, damaged,
    finish: () => releaseDownload(),
    fail: () => { rejectDownload = true; },
    allow: () => { rejectDownload = false; },
    emit: (payload) => listener({ payload }),
    close: () => { app.unmount(); assert.equal(unlistened, true); },
  };
}

test("download keeps the current model until success; unrelated progress is ignored", async () => {
  const f = await fixture();
  try {
    const pending = f.model.choose("best");
    assert.equal(f.store.ocrMode, "fast");
    assert.equal(f.model.inUse("fast"), true);
    assert.equal(f.model.inUse("best"), false);
    assert.deepEqual(f.calls, [{ mode: "best", activate: true }]);
    f.emit({ mode: "fast", phase: "downloading", downloadedBytes: 100, totalBytes: 100, networkBytes: 100 });
    assert.equal(f.model.percent.value, 0);
    f.emit({ mode: "best", phase: "verifying", downloadedBytes: 50, totalBytes: 100, networkBytes: 40 });
    assert.equal(f.model.percent.value, 50);
    await f.model.removeAll();
    assert.equal(f.available.has("fast"), true, "removal is blocked during download");
    f.finish();
    await pending;
    assert.equal(f.model.inUse("best"), true);
    assert.equal(f.model.installed("fast"), true);
    assert.equal(f.model.progress.value, null);
  } finally { f.close(); }
});

test("a failed download leaves the current model usable and can be retried", async () => {
  const f = await fixture();
  try {
    f.fail();
    const failed = f.model.choose("best");
    f.damaged.add("best");
    f.finish(); await failed;
    assert.equal(f.model.failure.value.detail, "Error: HTTP 403");
    assert.equal(f.model.inUse("fast"), true);
    assert.equal(f.model.busy.value, false);
    assert.equal(f.model.needsRepair("best"), true);
    f.allow();
    const retry = f.model.choose("best");
    f.finish(); await retry;
    assert.equal(f.model.failure.value, null);
    assert.equal(f.model.inUse("best"), true);
    assert.deepEqual(f.calls, [{ mode: "best", activate: true }, { mode: "best", activate: true }]);
  } finally { f.close(); }
});

test("the primary action repairs broken models without activating them; complete models switch without downloading", async () => {
  const f = await fixture();
  try {
    f.damaged.add("best");
    await f.model.load();
    assert.equal(f.model.needsRepair("best"), true);
    assert.equal(f.model.statuses.value.best.downloadedBytes, 0, "repair does not depend on having valid bytes");
    const repair = f.model.choose("best");
    f.finish(); await repair;
    assert.deepEqual(f.calls, [{ mode: "best", activate: false }]);
    assert.equal(f.model.inUse("fast"), true);
    assert.equal(f.model.needsRepair("best"), false);
    await f.model.choose("best");
    assert.equal(f.calls.length, 1);
    assert.equal(f.model.inUse("best"), true);
    await f.model.removeAll();
    assert.equal(f.model.hasModels.value, false);
    assert.equal(f.model.inUse("best"), false);
  } finally { f.close(); }
});

test("shared files do not turn an unused model's download action into repair", async () => {
  const f = await fixture();
  try {
    assert.ok(f.model.statuses.value.best.downloadedBytes > 0);
    assert.equal(f.model.needsRepair("best"), false);
    const download = f.model.choose("best");
    assert.deepEqual(f.calls, [{ mode: "best", activate: true }]);
    f.finish(); await download;
    assert.equal(f.model.inUse("best"), true);
  } finally { f.close(); }
});

test("a switch failure caused by damaged files retries as repair", async () => {
  const f = await fixture();
  try {
    f.available.add("best");
    await f.model.load();
    f.damaged.add("best");
    await f.model.choose("best");
    assert.equal(f.model.failure.value.kind, "switch");
    assert.equal(f.model.needsRepair("best"), true);
    assert.deepEqual(f.calls, []);
    const retry = f.model.choose("best");
    assert.deepEqual(f.calls, [{ mode: "best", activate: false }]);
    f.finish(); await retry;
    assert.equal(f.model.failure.value, null);
    assert.equal(f.model.installed("best"), true);
    assert.equal(f.store.ocrMode, "fast");
  } finally { f.close(); }
});
