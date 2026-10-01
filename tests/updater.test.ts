import assert from "node:assert/strict";
import { after, beforeEach, test } from "node:test";
import { createRenderer, h, markRaw } from "vue";
import UpdateDialog from "../src/components/UpdateDialog.vue";
import { useUpdater } from "../src/composables/useUpdater";

const state = globalThis.__updaterTest;
const flush = () => new Promise<void>(resolve => setImmediate(resolve));
const updater = useUpdater();

function deferred<T = void>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}

function node(tag = "", text = "") {
  return markRaw({ tag, text, props: {}, children: [], parent: null, focus() {} });
}
const renderer = createRenderer({
  createElement: tag => node(tag), createText: text => node("#text", text),
  createComment: () => node("#comment"),
  setText: (el, text) => { el.text = text; },
  setElementText: (el, text) => { el.text = text; el.children = []; },
  patchProp: (el, key, _, value) => { el.props[key] = value; },
  insert(el, parent, anchor) {
    if (el.parent) el.parent.children.splice(el.parent.children.indexOf(el), 1);
    el.parent = parent;
    const index = anchor ? parent.children.indexOf(anchor) : -1;
    if (index < 0) parent.children.push(el);
    else parent.children.splice(index, 0, el);
  },
  remove(el) {
    if (el.parent) el.parent.children.splice(el.parent.children.indexOf(el), 1);
    el.parent = null;
  },
  parentNode: el => el.parent,
  nextSibling: el => el.parent?.children[el.parent.children.indexOf(el) + 1] ?? null,
});

function text(el) { return el.text + el.children.map(text).join(""); }
function all(root, predicate) {
  return [...(predicate(root) ? [root] : []), ...root.children.flatMap(child => all(child, predicate))];
}
function button(root, label) {
  const result = all(root, el => el.tag === "button" && text(el) === label)[0];
  assert.ok(result, `button ${label} exists`);
  return result;
}
function mountDialog() {
  const root = node("root");
  const app = renderer.createApp({ render: () => h(UpdateDialog, {
    open: updater.updateDialogOpen.value,
    status: updater.updateStatus.value,
    update: updater.availableUpdate.value,
    error: updater.updateError.value,
    errorPhase: updater.updateErrorPhase.value,
    downloadedBytes: updater.updateDownloadedBytes.value,
    totalBytes: updater.updateTotalBytes.value,
    onDismiss: updater.dismissUpdateDialog,
    onInstall: updater.installAvailableUpdate,
    onRetry: updater.retryUpdate,
    onRelaunch: updater.relaunchForUpdate,
  }) });
  app.mount(root);
  return { root, close: () => app.unmount() };
}
function release(downloadAndInstall = async (_onEvent) => {}) {
  return { currentVersion: "0.2.15", version: "0.2.16", body: "Release notes", downloadAndInstall, close: async () => {} };
}

beforeEach(async context => {
  context.mock.method(console, "warn", () => {});
  state.check = async () => null;
  state.relaunch = async () => {};
  state.snapshots = [];
  updater.updateStatus.value = "idle";
  updater.dismissUpdateDialog();
  await updater.checkForUpdate({ silent: true });
});
after(() => updater.dispose());

test("reopened download errors can retry repeatedly, reset progress, and reject duplicate downloads", async () => {
  const attempts: ReturnType<typeof deferred>[] = [];
  let checkCalls = 0;
  const update = release(async onEvent => {
    const attempt = deferred();
    attempts.push(attempt);
    if (attempts.length === 1) {
      onEvent({ event: "Started", data: { contentLength: 100 } });
      onEvent({ event: "Progress", data: { chunkLength: 40 } });
      throw new Error("network unavailable");
    }
    await attempt.promise;
  });
  state.check = async () => { checkCalls++; return update; };
  await updater.checkForUpdate();
  await updater.installAvailableUpdate();
  assert.equal(updater.updateError.value, "update.error.downloadNetwork");
  assert.equal(updater.updateProgressPercent.value, 40);
  updater.dismissUpdateDialog();
  updater.openUpdateDialog();
  const dialog = mountDialog();
  try {
    const retry = button(dialog.root, "update.retry");
    retry.props.onClick();
    retry.props.onClick();
    await flush();
    assert.equal(attempts.length, 2);
    assert.equal(updater.updateStatus.value, "downloading");
    assert.equal(updater.updateError.value, null);
    assert.equal(updater.updateDownloadedBytes.value, 0);
    assert.equal(updater.updateTotalBytes.value, null);
    assert.equal(all(dialog.root, el => el.tag === "button" && text(el) === "update.retry").length, 0);
    attempts[1].reject(new Error("network still unavailable"));
    await flush();
    assert.equal(updater.updateStatus.value, "error");
    button(dialog.root, "update.retry").props.onClick();
    await flush();
    assert.equal(attempts.length, 3);
    attempts[2].resolve();
    await flush();
    assert.equal(updater.updateStatus.value, "ready");
    assert.equal(updater.updateProgressPercent.value, 100);
    button(dialog.root, "update.restartNow");
    assert.equal(checkCalls, 1, "download retries reuse the available update");
  } finally { attempts.forEach(attempt => attempt.resolve()); dialog.close(); }
});

for (const hasUpdate of [true, false]) {
  test(`failed checks retry through the dialog and finish with ${hasUpdate ? "an update" : "no update"}`, async () => {
    state.check = async () => { throw new Error("network unavailable"); };
    await updater.checkForUpdate();
    const dialog = mountDialog();
    const checking = deferred<ReturnType<typeof release> | null>();
    let checkCalls = 0;
    state.check = () => { checkCalls++; return checking.promise; };
    try {
      const retry = button(dialog.root, "update.retry");
      retry.props.onClick();
      retry.props.onClick();
      await flush();
      assert.equal(checkCalls, 1);
      assert.equal(updater.updateStatus.value, "checking");
      assert.ok([true, ""].includes(button(dialog.root, "update.button.checking").props.disabled));
      assert.ok(text(dialog.root).includes("update.summary.checking"));
      checking.resolve(hasUpdate ? release() : null);
      await flush();
      assert.equal(updater.updateStatus.value, hasUpdate ? "available" : "noUpdate");
      assert.equal(updater.updateError.value, null);
      if (hasUpdate) {
        button(dialog.root, "update.installNow");
      } else {
        assert.ok(text(dialog.root).includes("update.title.noUpdate"));
        assert.ok(text(dialog.root).includes("update.summary.noUpdate"));
        button(dialog.root, "common.gotIt").props.onClick();
        assert.equal(updater.updateDialogOpen.value, false);
      }
    } finally { checking.resolve(null); dialog.close(); }
  });
}

test("a relaunch error retries the restart without another check or download", async () => {
  let checkCalls = 0, downloads = 0, relaunches = 0;
  state.check = async () => { checkCalls++; return release(async () => { downloads++; }); };
  state.relaunch = async () => {
    relaunches++;
    if (relaunches === 1) throw new Error("restart failed");
  };
  await updater.checkForUpdate();
  await updater.installAvailableUpdate();
  await updater.relaunchForUpdate();
  const dialog = mountDialog();
  try {
    assert.equal(updater.updateErrorPhase.value, "relaunch");
    button(dialog.root, "update.retry").props.onClick();
    await flush();
    assert.equal(relaunches, 2);
    assert.equal(checkCalls, 1);
    assert.equal(downloads, 1);
  } finally { dialog.close(); }
});

test("a settings-window download retry uses the main resource and publishes its new state", async () => {
  let downloads = 0;
  state.check = async () => release(async () => {
    downloads++;
    if (downloads === 1) throw new Error("network unavailable");
  });
  await updater.checkForUpdate();
  await updater.installAvailableUpdate();
  state.onCommand({ payload: { action: "install", source: "settings" } });
  await flush();
  assert.equal(downloads, 2);
  assert.equal(updater.updateStatus.value, "ready");
  assert.ok(state.snapshots.some(({ snapshot }) => snapshot.status === "downloading" && snapshot.error === null));
  assert.equal(state.snapshots.at(-1).snapshot.status, "ready");
});
