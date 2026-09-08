import assert from "node:assert/strict";
import { test } from "node:test";
import { computed, createRenderer, markRaw, nextTick, reactive, ref } from "vue";
import SettingsWindow from "../src/components/SettingsWindow.vue";
import OcrSetupGuide from "../src/components/OcrSetupGuide.vue";
import { getOcrErrorMessage, getOcrModelIssue } from "../src/lib/ocrError";

const state = globalThis.__settingsTest;
function deferred<T = void>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}

const flush = () => new Promise<void>(resolve => setImmediate(resolve));

function node(tag = "", text = "") {
  return markRaw({
    tag, tagName: tag.toUpperCase(), text, props: {}, children: [], parent: null,
    style: {}, value: "", addEventListener() {}, removeEventListener() {},
    setAttribute(key, value) { this.props[key] = value; },
    removeAttribute(key) { delete this.props[key]; },
    focus() { globalThis.document.activeElement = this; },
  });
}
const renderer = createRenderer({
  createElement: (tag) => node(tag), createText: (text) => node("#text", text),
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
  parentNode: (el) => el.parent,
  nextSibling: (el) => el.parent?.children[el.parent.children.indexOf(el) + 1] ?? null,
});

function all(root, predicate) {
  return [...(predicate(root) ? [root] : []), ...root.children.flatMap(child => all(child, predicate))];
}
function text(el) {
  if (el.props["aria-hidden"] === true || el.props["aria-hidden"] === "true") return "";
  return el.text + el.children.map(text).join("");
}
function hasClass(el, name) { return String(el.props.class ?? "").split(" ").includes(name); }
function button(root, label) {
  const result = all(root, el => el.tag === "button" && (text(el).trim() === label || el.props["aria-label"] === label))[0];
  assert.ok(result, `button ${label} exists`);
  return result;
}

function ocrFixture() {
  const statuses = ref({
    fast: { installed: true, needsRepair: false, downloadedBytes: 6_932_119, hasResources: true },
    best: { installed: true, needsRepair: false, downloadedBytes: 31_824_456, hasResources: true },
  });
  const operation = ref(null), loading = ref(false), removing = ref(false);
  const installed = mode => statuses.value[mode].installed;
  state.ocrCalls = [];
  state.openedPaths = [];
  state.openedUrls = [];
  return {
    statuses, operation, loading, removing,
    busy: computed(() => loading.value || removing.value || operation.value !== null),
    hasModels: computed(() => Object.values(statuses.value).some(status => status.hasResources)),
    installDir: ref("C:\\Users\\Test\\AppData\\Roaming\\com.ipaste.desktop\\ocr\\models"),
    progress: ref(null), failure: ref(null), error: ref(null), notice: ref(null),
    percent: ref(0), bytesPerSecond: ref(0), installed,
    inUse: mode => installed(mode) && state.store.ocrMode === mode,
    needsRepair: mode => statuses.value[mode].needsRepair,
    async load() { state.ocrCalls.push(["load"]); },
    async choose(mode) { state.ocrCalls.push(["choose", mode]); state.store.ocrMode = mode; },
    async removeAll() { state.ocrCalls.push(["removeAll"]); },
  };
}

function fixture(options: { initialTab?: "ocr"; queuedTarget?: "ocr" } = {}) {
  const preferences = deferred(), autostart = deferred<boolean>();
  let preferenceReads = 0, autostartReads = 0, snapshotReads = 0;
  state.ocrMounts = 0;
  window.location.search = options.initialTab ? "?window=settings&tab=ocr" : "?window=settings";
  state.settingsTarget = options.queuedTarget ?? null;
  state.settingsListener = null;
  state.listen = async (event, listener) => {
    assert.equal(event, "ipaste://settings-navigation");
    state.settingsListener = listener;
    return () => { if (state.settingsListener === listener) state.settingsListener = null; };
  };
  state.writes = 0;
  state.readAutostart = () => { autostartReads++; return autostart.promise; };
  state.api = {
    async appInfo() { return { version: "test" }; },
    async takeSettingsTarget() {
      const target = state.settingsTarget;
      state.settingsTarget = null;
      return target;
    },
  };
  state.store = reactive({
    language: "en", retentionDays: 30, appendCopyTimeoutMinutes: 1,
    panelOpenBehavior: "history", panelLayout: "top", ocrMode: "fast",
    shortcut: "CommandOrControl+Shift+V", cloud: { enabled: false, apiAddress: "", apiKey: "" },
    async loadSettings() { preferenceReads++; await preferences.promise; },
    async load() { snapshotReads++; },
  });
  state.ocr = ocrFixture();
  const root = node("root");
  const app = renderer.createApp(SettingsWindow);
  app.mount(root);
  async function select(tab) {
    const button = all(root, el => el.tag === "button" && hasClass(el, "settings-tab") && text(el) === `settings.tabs.${tab}`)[0];
    assert.ok(button, `tab ${tab} exists`);
    button.props.onClick();
    await nextTick();
  }
  return { app, root, select, preferences, autostart, reads: () => ({ preferenceReads, autostartReads, snapshotReads }) };
}

test("opening settings performs no OCR mount or clipboard snapshot; autostart reads independently", async () => {
  const f = fixture();
  try {
    assert.equal(state.ocrMounts, 0);
    assert.deepEqual(f.reads(), { preferenceReads: 1, autostartReads: 1, snapshotReads: 0 });
    assert.equal(all(f.root, el => el.props.role === "switch").length, 0, "unknown state must not look disabled");
    assert.equal(all(f.root, el => hasClass(el, "autostart-pending")).length, 1);
    f.autostart.resolve(true);
    await flush();
    const control = all(f.root, el => el.props.role === "switch")[0];
    assert.equal(control.props["aria-checked"], true);
    assert.equal(state.writes, 0, "reading must not enable or disable autostart");
  } finally { f.preferences.resolve(); f.app.unmount(); }
});

test("OCR and About are mutually exclusive, and revisiting OCR retains its component", async () => {
  const f = fixture();
  try {
    await f.select("ocr");
    assert.equal(state.ocrMounts, 1);
    assert.equal(all(f.root, el => hasClass(el, "settings-about-panel")).length, 0);
    const ocr = all(f.root, el => hasClass(el, "ocr-settings"))[0];
    assert.notEqual(ocr.style.display, "none");
    await f.select("about");
    assert.equal(all(f.root, el => hasClass(el, "settings-about-panel")).length, 1);
    assert.equal(ocr.style.display, "none");
    await f.select("ocr");
    assert.equal(state.ocrMounts, 1, "switching tabs must not remount a downloading panel");
    assert.equal(all(f.root, el => hasClass(el, "settings-about-panel")).length, 0);
    assert.notEqual(ocr.style.display, "none");
  } finally { f.preferences.resolve(); f.autostart.resolve(false); f.app.unmount(); }
});

test("all Windows tabs can be selected while startup requests are pending", async () => {
  const f = fixture();
  try {
    for (const tab of ["shortcuts", "dataManagement", "about", "ocr", "general"]) {
      await f.select(tab);
      const active = all(f.root, el => hasClass(el, "settings-tab-active"));
      assert.equal(active.length, 1);
      assert.equal(text(active[0]), `settings.tabs.${tab}`);
      assert.equal(all(f.root, el => hasClass(el, "settings-about-panel")).length, tab === "about" ? 1 : 0);
    }
  } finally { f.preferences.resolve(); f.autostart.resolve(false); f.app.unmount(); }
});

test("failed autostart reads stay unknown and can retry without changing the system setting", async () => {
  const f = fixture();
  try {
    f.autostart.reject(new Error("registry unavailable"));
    await flush();
    assert.equal(all(f.root, el => el.props.role === "switch").length, 0);
    assert.equal(all(f.root, el => el.props.role === "alert").length, 1);
    const pending = all(f.root, el => hasClass(el, "autostart-pending"))[0];
    const retry = all(pending, el => el.tag === "button")[0];
    assert.equal(text(retry), "settings.retry");
    state.readAutostart = async () => false;
    await retry.props.onClick();
    await nextTick();
    assert.equal(all(f.root, el => el.props.role === "switch")[0].props["aria-checked"], false);
    assert.equal(all(f.root, el => el.props.role === "alert").length, 0);
    assert.equal(state.writes, 0);
  } finally { f.preferences.resolve(); f.app.unmount(); }
});

test("failed preference reads offer retry without blocking autostart or OCR navigation", async () => {
  const f = fixture();
  try {
    f.preferences.reject(new Error("database unavailable"));
    f.autostart.resolve(true);
    await flush();
    const alert = all(f.root, el => el.props.role === "alert")[0];
    assert.ok(text(alert).includes("settings.loadError"));
    assert.equal(all(f.root, el => el.props.role === "switch")[0].props["aria-checked"], true);
    let retries = 0;
    state.store.loadSettings = async () => { retries++; };
    await all(alert, el => el.tag === "button")[0].props.onClick();
    await nextTick();
    assert.equal(retries, 1);
    assert.equal(all(f.root, el => el.props.role === "alert").length, 0);
    await f.select("ocr");
    assert.equal(state.ocrMounts, 1);
    assert.equal(all(f.root, el => hasClass(el, "settings-about-panel")).length, 0);
  } finally { f.app.unmount(); }
});

test("normal OCR cards only show model selection, without a separate repair button", async () => {
  const f = fixture();
  try {
    await f.select("ocr");
    const panel = all(f.root, el => hasClass(el, "ocr-settings"))[0];
    assert.ok(hasClass(panel, "settings-panel"));
    const [fast, best] = all(panel, el => hasClass(el, "ocr-card"));
    assert.ok(hasClass(fast, "ocr-card-active"));
    assert.equal(all(panel, el => el.props.role === "progressbar").length, 0, "downloaded models have no progress bar");
    assert.equal(all(panel, el => hasClass(el, "settings-icon-button")).length, 0);
    assert.equal(all(fast, el => el.tag === "button").length, 0, "the current model needs no action");
    assert.equal(all(best, el => el.tag === "button").length, 1);
    const use = button(best, "ocr.useModel");
    assert.ok(hasClass(use, "settings-action-button"));
    await use.props.onClick();
    await nextTick();
    assert.ok(hasClass(best, "ocr-card-active"));
    assert.deepEqual(state.ocrCalls, [["choose", "best"]]);
    await button(panel, "ocr.openDownloadDir").props.onClick();
    await button(panel, "ocr.source").props.onClick();
    assert.deepEqual(state.openedPaths, [state.ocr.installDir.value]);
    assert.deepEqual(state.openedUrls, ["https://www.modelscope.cn/models/RapidAI/RapidOCR"]);
  } finally { f.preferences.resolve(); f.autostart.resolve(false); f.app.unmount(); }
});

test("OCR has one primary action for download, repair, or retry according to model status", async () => {
  const f = fixture();
  try {
    state.ocr.statuses.value.best = { installed: false, needsRepair: false, downloadedBytes: 500, hasResources: true };
    await f.select("ocr");
    const best = all(f.root, el => hasClass(el, "ocr-card"))[1];
    assert.ok(button(best, "ocr.downloadAndUse"));
    state.ocr.statuses.value.best.needsRepair = true;
    await nextTick();
    await button(best, "ocr.repairModel").props.onClick();
    assert.deepEqual(state.ocrCalls, [["choose", "best"]]);
    assert.equal(all(best, el => el.tag === "button").length, 1);
    state.ocr.failure.value = { mode: "best", kind: "repair", detail: "HTTP 403" };
    await nextTick();
    assert.ok(button(best, "ocr.retry"));
    assert.equal(all(best, el => el.tag === "button").length, 1);
  } finally { f.preferences.resolve(); f.autostart.resolve(false); f.app.unmount(); }
});

test("OCR progress fills only the downloading card and disables conflicting operations", async () => {
  const f = fixture();
  try {
    await f.select("ocr");
    state.store.ocrMode = "best";
    state.ocr.operation.value = { mode: "best", kind: "install" };
    state.ocr.progress.value = { phase: "downloading", downloadedBytes: 420, totalBytes: 1000 };
    state.ocr.percent.value = 42;
    await nextTick();
    const [fast, best] = all(f.root, el => hasClass(el, "ocr-card"));
    assert.equal(all(fast, el => hasClass(el, "ocr-progress-fill")).length, 0);
    assert.equal(all(best, el => hasClass(el, "ocr-progress-fill"))[0].props.style.transform, "scaleX(0.42)");
    assert.equal(all(best, el => el.props.role === "progressbar")[0].props["aria-valuenow"], 42);
    assert.equal(button(fast, "ocr.useModel").props.disabled, true);
    assert.equal(button(f.root, "ocr.deleteResources").props.disabled, true);
  } finally { f.preferences.resolve(); f.autostart.resolve(false); f.app.unmount(); }
});

test("OCR removal confirms inside the same button and resets when the mouse leaves", async () => {
  const f = fixture();
  try {
    await f.select("ocr");
    const remove = button(f.root, "ocr.deleteResources");
    await remove.props.onClick();
    assert.equal(button(f.root, "ocr.confirmRemove"), remove);
    assert.ok(hasClass(remove, "ocr-remove-confirm"));
    assert.equal(all(f.root, el => hasClass(el, "ocr-removal")).length, 0, "no confirmation panel is inserted");
    assert.deepEqual(state.ocrCalls, []);
    remove.props.onMouseleave();
    await nextTick();
    assert.equal(button(f.root, "ocr.deleteResources"), remove);
    assert.equal(hasClass(remove, "ocr-remove-confirm"), false);
    await remove.props.onClick();
    assert.deepEqual(state.ocrCalls, [], "returning to the button requires confirmation again");
    await button(f.root, "ocr.confirmRemove").props.onClick();
    assert.deepEqual(state.ocrCalls, [["removeAll"]]);
    assert.equal(button(f.root, "ocr.deleteResources"), remove);
  } finally { f.preferences.resolve(); f.autostart.resolve(false); f.app.unmount(); }
});

test("OCR removal confirmation resets on blur and Escape, and removal cannot be submitted twice", async () => {
  const f = fixture();
  const removal = deferred();
  state.ocr.removeAll = async () => {
    state.ocrCalls.push(["removeAll"]);
    state.ocr.removing.value = true;
    try { await removal.promise; }
    finally { state.ocr.removing.value = false; }
  };
  try {
    await f.select("ocr");
    const remove = button(f.root, "ocr.deleteResources");
    await remove.props.onClick();
    remove.props.onBlur();
    await nextTick();
    assert.equal(button(f.root, "ocr.deleteResources"), remove);
    await remove.props.onClick();
    remove.props.onKeydown({ key: "Escape", stopPropagation() {} });
    await nextTick();
    assert.equal(button(f.root, "ocr.deleteResources"), remove);
    assert.deepEqual(state.ocrCalls, []);
    await remove.props.onClick();
    const inFlight = remove.props.onClick();
    await nextTick();
    assert.equal(button(f.root, "ocr.deleting"), remove);
    assert.equal(remove.props.disabled, true);
    assert.equal(remove.props["aria-busy"], true);
    remove.props.onMouseleave();
    await remove.props.onClick();
    assert.deepEqual(state.ocrCalls, [["removeAll"]]);
    removal.resolve();
    await inFlight;
    await nextTick();
    assert.equal(button(f.root, "ocr.deleteResources"), remove);
    assert.equal(remove.props.disabled, false);
  } finally { removal.resolve(); f.preferences.resolve(); f.autostart.resolve(false); f.app.unmount(); }
});

test("OCR removal remains disabled when no models exist", async () => {
  const f = fixture();
  state.ocr.statuses.value = {
    fast: { installed: false, needsRepair: false, downloadedBytes: 0, hasResources: false },
    best: { installed: false, needsRepair: false, downloadedBytes: 0, hasResources: false },
  };
  try {
    await f.select("ocr");
    const remove = button(f.root, "ocr.deleteResources");
    assert.equal(remove.props.disabled, true);
    await remove.props.onClick();
    assert.equal(button(f.root, "ocr.deleteResources"), remove);
    assert.deepEqual(state.ocrCalls, []);
  } finally { f.preferences.resolve(); f.autostart.resolve(false); f.app.unmount(); }
});

test("a new settings window opens directly on the OCR tab", async () => {
  const f = fixture({ initialTab: "ocr" });
  try {
    assert.equal(state.ocrMounts, 1);
    assert.equal(text(all(f.root, el => hasClass(el, "settings-tab-active"))[0]), "settings.tabs.ocr");
    assert.equal(all(f.root, el => hasClass(el, "settings-about-panel")).length, 0);
  } finally { f.preferences.resolve(); f.autostart.resolve(false); f.app.unmount(); }
});

test("an OCR navigation request queued before the listener is ready is consumed on startup", async () => {
  const f = fixture({ queuedTarget: "ocr" });
  try {
    await flush();
    assert.equal(text(all(f.root, el => hasClass(el, "settings-tab-active"))[0]), "settings.tabs.ocr");
    assert.equal(state.settingsTarget, null);
    assert.equal(state.ocrMounts, 1);
  } finally { f.preferences.resolve(); f.autostart.resolve(false); f.app.unmount(); }
});

test("OCR navigation switches an existing window without remounting or interrupting downloads", async () => {
  const f = fixture();
  try {
    await flush();
    await f.select("ocr");
    await f.select("about");
    state.settingsTarget = "ocr";
    state.settingsListener();
    await flush();
    assert.equal(text(all(f.root, el => hasClass(el, "settings-tab-active"))[0]), "settings.tabs.ocr");
    assert.deepEqual(state.ocrCalls, [["load"]], "refresh file status when returning from the guide");
    assert.equal(state.ocrMounts, 1);
    state.ocr.operation.value = { mode: "best", kind: "install" };
    await f.select("general");
    state.settingsTarget = "ocr";
    state.settingsListener();
    await flush();
    assert.equal(text(all(f.root, el => hasClass(el, "settings-tab-active"))[0]), "settings.tabs.ocr");
    assert.equal(state.ocrMounts, 1);
    assert.deepEqual(state.ocrCalls, [["load"]], "do not recheck or reset an ongoing operation");
  } finally { f.preferences.resolve(); f.autostart.resolve(false); f.app.unmount(); }
  assert.equal(state.settingsListener, null, "navigation listener is cleaned up");
});

test("OCR model error codes distinguish setup from repair without parsing technical text", () => {
  assert.deepEqual(getOcrModelIssue({ code: "models_missing", missingFiles: ["PP-OCRv6_det_small.onnx"] }), {
    kind: "download", missingFiles: ["PP-OCRv6_det_small.onnx"],
  });
  assert.deepEqual(getOcrModelIssue({ code: "models_incomplete", missingFiles: ["model.onnx", null, 3] }), {
    kind: "repair", missingFiles: ["model.onnx"],
  });
  assert.equal(getOcrModelIssue({ code: "recognition_failed", message: "image file missing" }), null);
  assert.equal(getOcrModelIssue("model.onnx"), null);
  assert.equal(getOcrErrorMessage({ code: "recognition_failed", message: "image file missing" }), "image file missing");
  assert.equal(getOcrErrorMessage(new Error("read failed")), "read failed");
});

test("the missing-model guide hides filenames and opens OCR settings only when clicked", async () => {
  const requests = [];
  state.api = { async showSettings(tab) { requests.push(tab); } };
  const root = node("root");
  const app = renderer.createApp(OcrSetupGuide, { issue: { kind: "download", missingFiles: ["PP-OCRv6_det_small.onnx"] } });
  app.mount(root);
  try {
    assert.ok(text(root).includes("ocr.setup.downloadHint"));
    assert.equal(text(root).includes("PP-OCR"), false);
    assert.equal(all(root, el => el.tag === "details").length, 0);
    assert.deepEqual(requests, []);
    await button(root, "ocr.setup.downloadAction").props.onClick();
    assert.deepEqual(requests, ["ocr"]);
  } finally { app.unmount(); }
});

test("the repair guide folds diagnostics and offers a fallback if opening settings fails", async () => {
  const opening = deferred();
  let requests = 0;
  state.api = { async showSettings() { requests++; await opening.promise; } };
  const root = node("root");
  const app = renderer.createApp(OcrSetupGuide, { issue: { kind: "repair", missingFiles: ["broken.onnx"] } });
  app.mount(root);
  try {
    assert.ok(text(root).includes("ocr.setup.repairHint"));
    const details = all(root, el => el.tag === "details")[0];
    assert.equal(details.props.open, undefined);
    assert.ok(text(details).includes("broken.onnx"));
    const action = button(root, "ocr.setup.repairAction");
    const request = action.props.onClick();
    await nextTick();
    assert.equal(button(root, "ocr.setup.opening"), action);
    assert.equal(action.props.disabled, true);
    await action.props.onClick();
    assert.equal(requests, 1);
    opening.reject(new Error("IPC unavailable"));
    await request;
    await nextTick();
    assert.equal(text(all(root, el => el.props.role === "alert")[0]), "ocr.setup.openError");
    assert.equal(action.props.disabled, false);
    assert.equal(button(root, "ocr.setup.repairAction"), action);
  } finally { opening.resolve(); app.unmount(); }
});
