import assert from "node:assert/strict";
import { test } from "node:test";
import { computed, createRenderer, effectScope, nextTick, shallowRef } from "vue";
import { clipColumnCount, selectionDelta, submenuPlacement } from "../src/lib/panelLayout";
import { usePanelViewport } from "../src/composables/usePanelViewport";
import { useElementWidth } from "../src/composables/useElementWidth";

test("top layout switches only below 320, keeping 320 and 500 at two columns", () => {
  for (const width of [280, 319, 319.5]) assert.equal(clipColumnCount(width), 1);
  for (const width of [320, 321, 400, 500, 560, 720]) assert.equal(clipColumnCount(width), 2);
});

test("side layout subtracts the measured sidebar width before applying the same breakpoint", () => {
  for (const sidebarWidth of [156, 180]) {
    assert.equal(clipColumnCount(sidebarWidth + 319, sidebarWidth), 1);
    assert.equal(clipColumnCount(sidebarWidth + 320, sidebarWidth), 2);
    assert.equal(clipColumnCount(500, sidebarWidth), 2);
    assert.equal(selectionDelta("ArrowDown", clipColumnCount(400, sidebarWidth)), 1);
    assert.equal(selectionDelta("ArrowDown", clipColumnCount(500, sidebarWidth)), 2);
  }
});

test("sidebar measurements update columns when layout or sidebar size changes", async () => {
  let measuredWidth = 156;
  let notifyResize!: () => void;
  let disconnected = false;
  const previousObserver = globalThis.ResizeObserver;
  globalThis.ResizeObserver = class {
    constructor(callback) { notifyResize = callback; }
    observe() {}
    disconnect() { disconnected = true; }
  } as any;
  const scope = effectScope();
  try {
    const target = shallowRef<HTMLElement | null>(null);
    const side = shallowRef(false);
    const columns = scope.run(() => {
      const width = useElementWidth(target);
      return computed(() => clipColumnCount(476, side.value ? width.value : 0));
    })!;
    target.value = { getBoundingClientRect: () => ({ width: measuredWidth }) } as HTMLElement;
    await nextTick();
    assert.equal(columns.value, 2);
    side.value = true;
    assert.equal(columns.value, 2); // Exactly 320px remains after the sidebar.
    measuredWidth = 157;
    notifyResize();
    assert.equal(columns.value, 1);
    side.value = false;
    assert.equal(columns.value, 2);
  } finally {
    scope.stop();
    globalThis.ResizeObserver = previousObserver;
  }
  assert.equal(disconnected, true);
});

test("arrow navigation follows one or two columns without skipping narrow rows", () => {
  for (const columns of [1, 2]) {
    assert.equal(selectionDelta("ArrowDown", columns), columns);
    assert.equal(selectionDelta("ArrowUp", columns), -columns);
    assert.equal(selectionDelta("ArrowRight", columns), 1);
    assert.equal(selectionDelta("ArrowLeft", columns), -1);
  }
  assert.equal(selectionDelta("Enter", 2), null);
});

test("submenus expand inline when neither edge has enough space", () => {
  assert.deepEqual(submenuPlacement({ left: 20, right: 166 }, 168, 280), { inline: true, alignLeft: false });
  assert.deepEqual(submenuPlacement({ left: 20, right: 166 }, 168, 319), { inline: true, alignLeft: false });
  assert.deepEqual(submenuPlacement({ left: 20, right: 166 }, 168, 560), { inline: false, alignLeft: false });
  assert.deepEqual(submenuPlacement({ left: 390, right: 536 }, 168, 560), { inline: false, alignLeft: true });
  assert.deepEqual(submenuPlacement({ left: 130, right: 276 }, 168, 400), { inline: true, alignLeft: false });
});

test("resize persistence coalesces changes, flushes before hide and removes listeners", async () => {
  const listeners = new Map<string, () => void>();
  const timers = new Map<number, () => void>();
  const writes: unknown[] = [];
  let timerId = 0;
  globalThis.window = {
    innerWidth: 560, innerHeight: 620, __TAURI_INTERNALS__: {},
    addEventListener: (name, listener) => listeners.set(name, listener),
    removeEventListener: name => listeners.delete(name),
    setTimeout: callback => { timers.set(++timerId, callback); return timerId; },
    clearTimeout: id => timers.delete(id),
  } as any;
  globalThis.__panelInvoke = async (command, payload) => { writes.push({ command, ...payload }); };
  const noop = () => {};
  const renderer = createRenderer({
    createElement: () => ({}), createText: () => ({}), createComment: () => ({}),
    setText: noop, setElementText: noop, patchProp: noop, insert: noop, remove: noop,
    parentNode: () => null, nextSibling: () => null,
  });
  let viewport!: ReturnType<typeof usePanelViewport>;
  let resizeCount = 0;
  const app = renderer.createApp({
    setup() { viewport = usePanelViewport(true, () => resizeCount++); return () => null; },
  });
  app.mount({});
  for (const width of [400, 319, 280]) {
    window.innerWidth = width;
    window.innerHeight = 740;
    listeners.get("resize")!();
  }
  assert.equal(viewport.width.value, 280);
  assert.equal(resizeCount, 3);
  assert.equal(timers.size, 1);
  assert.equal(writes.length, 0);
  await viewport.flushSize();
  assert.deepEqual(writes, [{ command: "save_main_window_size", size: { width: 280, height: 740 } }]);
  assert.equal(timers.size, 0);
  window.innerWidth = 0;
  listeners.get("resize")!();
  await viewport.flushSize();
  assert.equal(writes.length, 1);
  window.innerWidth = 320;
  listeners.get("resize")!();
  app.unmount();
  await viewport.flushSize();
  assert.equal(writes.length, 2);
  assert.equal(listeners.size, 0);
  assert.equal(timers.size, 0);
});
