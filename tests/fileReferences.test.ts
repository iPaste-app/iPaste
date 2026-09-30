import assert from "node:assert/strict";
import { test } from "node:test";
import { createRenderer, h, markRaw, nextTick, ref } from "vue";
import ClipCard from "../src/components/ClipCard.vue";
import ClipViewerWindow from "../src/components/ClipViewerWindow.vue";
import { clipFileExtension, clipFileName, fileIconSrc } from "../src/lib/clipFile";
import { localizeFileError } from "../src/lib/fileError";
import { clipMetricText, formatFileSize } from "../src/lib/format";
import type { ClipViewItem } from "../src/types";
import { ipasteApi as browserFileApi } from "@file-test/api-browser";
import { ipasteApi as nativeFileApi } from "@file-test/api-native";

type Props = Record<string, unknown>;

const state = (globalThis as typeof globalThis & {
  __fileReferencesTest: {
    clipboardWrites: string[];
    invokeCalls: Array<{ command: string; args: Record<string, unknown> | undefined }>;
    storage: Map<string, string>;
    viewerApi: Record<string, (...args: unknown[]) => unknown>;
  };
}).__fileReferencesTest;

const fileReference = String.raw`C:\Users\Lin Zhao\Documents\设计 文档\合同 Final.PDF`;

class HostNode {
  props: Props = {};
  children: HostNode[] = [];
  parent: HostNode | null = null;
  scrollLeft = 0;
  scrollTop = 0;
  selectionStart = 0;
  selectionEnd = 0;
  style = {
    removeProperty: () => {},
    setProperty: () => {},
  };

  constructor(public tag = "", public text = "") {
    markRaw(this);
  }

  get tagName() {
    return this.tag.toUpperCase();
  }

  addEventListener() {}
  removeEventListener() {}
  setAttribute(key: string, value: unknown) { this.props[key] = value; }
  removeAttribute(key: string) { delete this.props[key]; }
  setPointerCapture() {}
  releasePointerCapture() {}
  setSelectionRange(start: number, end: number) {
    this.selectionStart = start;
    this.selectionEnd = end;
  }
  focus() { globalThis.document.activeElement = this as never; }
  scrollIntoView() {}
  getBoundingClientRect() {
    return { x: 0, y: 0, top: 0, right: 320, bottom: 120, left: 0, width: 320, height: 120 };
  }
}

const renderer = createRenderer({
  createElement: (tag: string) => new HostNode(tag),
  createText: (text: string) => new HostNode("#text", text),
  createComment: () => new HostNode("#comment"),
  setText: (element: HostNode, text: string) => { element.text = text; },
  setElementText: (element: HostNode, text: string) => {
    element.text = text;
    element.children = [];
  },
  patchProp: (element: HostNode, key: string, _previous: unknown, value: unknown) => {
    element.props[key] = value;
  },
  insert(element: HostNode, parent: HostNode, anchor: HostNode | null) {
    if (element.parent) {
      const previousIndex = element.parent.children.indexOf(element);
      if (previousIndex >= 0) element.parent.children.splice(previousIndex, 1);
    }
    element.parent = parent;
    const index = anchor ? parent.children.indexOf(anchor) : -1;
    if (index < 0) parent.children.push(element);
    else parent.children.splice(index, 0, element);
  },
  remove(element: HostNode) {
    if (!element.parent) return;
    const index = element.parent.children.indexOf(element);
    if (index >= 0) element.parent.children.splice(index, 1);
    element.parent = null;
  },
  parentNode: (element: HostNode) => element.parent,
  nextSibling: (element: HostNode) => {
    const parent = element.parent;
    return parent?.children[parent.children.indexOf(element) + 1] ?? null;
  },
});

function all(root: HostNode, predicate: (element: HostNode) => boolean): HostNode[] {
  return [
    ...(predicate(root) ? [root] : []),
    ...root.children.flatMap((child) => all(child, predicate)),
  ];
}

function hasClass(element: HostNode, className: string) {
  return String(element.props.class ?? "").split(" ").includes(className);
}

function nodeText(element: HostNode): string {
  if (element.props["aria-hidden"] === true || element.props["aria-hidden"] === "true") return "";
  return element.text + element.children.map(nodeText).join("");
}

function event(target: HostNode) {
  return {
    currentTarget: target,
    target,
    preventDefault() {},
    stopPropagation() {},
  };
}

async function flushFilePreviewUpdate() {
  await nextTick();
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  await nextTick();
}

function historyFile(path = fileReference): ClipViewItem {
  return {
    collection: "history",
    id: "file-1",
    clipType: "file",
    contentHash: "file-hash",
    previewText: path,
    text: path,
    lastCapturedAt: "2026-09-22T00:00:00Z",
    favoriteCount: 0,
    isPinned: false,
  };
}

test("file helpers preserve Windows and POSIX names while handling unknown extensions and dotfiles", () => {
  const paths = [
    { path: String.raw`C:\Users\王 小明\Desktop\预算 2026.XLSX`, name: "预算 2026.XLSX", extension: "xlsx" },
    { path: String.raw`\\server\share\资料\预算.xlsx`, name: "预算.xlsx", extension: "xlsx" },
    { path: "/Users/李 雷/资料/合同 final.pdf", name: "合同 final.pdf", extension: "pdf" },
    { path: "/srv/releases/build artifact.unknown", name: "build artifact.unknown", extension: "unknown" },
    { path: "/var/tmp/README", name: "README", extension: "" },
    { path: String.raw`C:\workspace\.env`, name: ".env", extension: "" },
  ];

  for (const fixture of paths) {
    assert.equal(clipFileName(fixture.path), fixture.name, fixture.path);
    assert.equal(clipFileExtension(fixture.path), fixture.extension, fixture.path);
    assert.ok(fileIconSrc(fixture.path), `a generic or specialized icon is available for ${fixture.path}`);
  }

  const posixLiteralBackslash = "/tmp/snapshots/one\\two.ts";
  assert.equal(clipFileName(posixLiteralBackslash), "one\\two.ts");
  assert.equal(clipFileExtension(posixLiteralBackslash), "ts");
  assert.equal(fileIconSrc(posixLiteralBackslash), "/file-icons/code.webp");
  assert.equal(fileIconSrc("/tmp/report.PDF"), "/file-icons/pdf.webp");
  assert.equal(fileIconSrc("/tmp/report.docx"), "/file-icons/document.webp");
  assert.equal(fileIconSrc("/tmp/file.unknown"), "/file-icons/generic.webp");
});

test("file sizes choose readable units and omit unavailable metadata", () => {
  for (const [bytes, expected] of [
    [0, "0 B"], [512, "512 B"], [1024, "1 KB"], [1536, "1.5 KB"],
    [1024 ** 2, "1 MB"], [2.4 * 1024 ** 2, "2.4 MB"],
    [1024 ** 3, "1 GB"], [1024 ** 4, "1 TB"], [1024 ** 2 - 1, "1 MB"],
    [null, ""], [undefined, ""], [-1, ""], [NaN, ""], [Infinity, ""],
  ] as const) assert.equal(formatFileSize(bytes), expected);
  assert.equal(clipMetricText("file", fileReference, "", 1536), "1.5 KB · PDF");
  assert.equal(clipMetricText("file", fileReference, "", 0), "0 B · PDF");
  assert.equal(clipMetricText("file", fileReference, "", null), "PDF");
});

test("a file card loads preview metadata and ignores stale results after its reference changes", async () => {
  let resolveOld: (preview: { size: number; thumbnailPath: string | null }) => void = () => {};
  const item = ref(historyFile());
  state.viewerApi.fileReferencePreview = (path) => path === fileReference
    ? new Promise<{ size: number; thumbnailPath: string | null }>((resolve) => { resolveOld = resolve; })
    : Promise.resolve({ size: 1536, thumbnailPath: null });
  const root = new HostNode("root");
  const app = renderer.createApp({ render: () => h(ClipCard, {
    item: item.value, index: 0, selected: false, categoryTags: [],
    editingName: null, reorderEnabled: false,
  }) });
  app.mount(root);
  const metric = () => nodeText(all(root, (node) => hasClass(node, "clip-metric-badge"))[0]);
  try {
    assert.equal(metric(), "PDF");
    item.value = historyFile("C:\\files\\archive.zip");
    await flushFilePreviewUpdate();
    assert.equal(metric(), "1.5 KB · ZIP");
    resolveOld({ size: 1024 ** 3, thumbnailPath: "C:\\cache\\stale.png" });
    await flushFilePreviewUpdate();
    assert.equal(metric(), "1.5 KB · ZIP");
    state.viewerApi.fileReferencePreview = async () => { throw "FILE_NOT_FOUND"; };
    item.value = historyFile("C:\\files\\missing.pdf");
    await flushFilePreviewUpdate();
    assert.equal(metric(), "PDF");
    const failure = all(root, (node) => hasClass(node, "clip-file-error"))[0];
    assert.equal(nodeText(failure), "file.error.notFound");
  } finally {
    app.unmount();
  }
});

test("an image file card uses only the generated thumbnail and clears it when the source disappears", async () => {
  const imagePath = String.raw`C:\photos\holiday.jpg`;
  const item = ref(historyFile(imagePath));
  state.viewerApi.fileReferencePreview = async () => ({
    size: 2048,
    thumbnailPath: String.raw`C:\cache\file-previews\holiday.webp`,
    dimensions: { width: 4032, height: 3024 },
  });
  const root = new HostNode("root");
  const app = renderer.createApp({ render: () => h(ClipCard, {
    item: item.value, index: 0, selected: false, categoryTags: [],
    editingName: null, reorderEnabled: false,
  }) });
  app.mount(root);
  await flushFilePreviewUpdate();

  try {
    const imagePreview = all(root, (node) => hasClass(node, "clip-preview-image"))[0];
    assert.ok(imagePreview, "image files share the regular image preview layout");
    const thumbnail = all(imagePreview, (node) => node.tag === "img")[0];
    assert.ok(thumbnail, "supported image files render a generated thumbnail");
    assert.equal(thumbnail.props.src, String.raw`C:\cache\file-previews\holiday.webp`);
    assert.notEqual(thumbnail.props.src, imagePath, "the original external path is never used as an image source");
    const metric = all(root, (node) => hasClass(node, "clip-metric-badge"))[0];
    assert.equal(nodeText(metric), "2 KB, 4032 × 3024", "assistive technology can read both measurements");
    assert.equal(all(metric, (node) => hasClass(node, "clip-file-metric-size"))[0].text, "2 KB");
    assert.equal(all(metric, (node) => hasClass(node, "clip-file-metric-pixels"))[0].text, "4032 × 3024");
    const footer = all(root, (node) => hasClass(node, "clip-card-footer"))[0];
    assert.equal(nodeText(all(footer, (node) => hasClass(node, "clip-file-footer-name"))[0]), "holiday.jpg");
    assert.equal(all(root, (node) => hasClass(node, "clip-file-name")).length, 0);

    state.viewerApi.fileReferencePreview = async () => { throw "FILE_NOT_FOUND"; };
    item.value = { ...historyFile(imagePath), lastCapturedAt: "2026-09-22T00:00:01Z" };
    await flushFilePreviewUpdate();
    assert.equal(all(root, (node) => hasClass(node, "clip-preview-image")).length, 0);
    assert.equal(all(root, (node) => hasClass(node, "clip-file-metric-pixels")).length, 0);
    assert.equal(nodeText(all(root, (node) => hasClass(node, "clip-file-error"))[0]), "file.error.notFound");
    assert.equal(nodeText(all(root, (node) => hasClass(node, "clip-file-footer-name"))[0]), "holiday.jpg");
  } finally {
    app.unmount();
  }
});

test("a file card shows its filename, keeps the path in its tooltip, and applies the untruncated reference", () => {
  const item = historyFile();
  const applied: ClipViewItem[] = [];
  const root = new HostNode("root");
  const app = renderer.createApp({
    render: () => h(ClipCard, {
      item,
      index: 0,
      selected: true,
      categoryTags: [],
      editingName: null,
      reorderEnabled: false,
      onApply: (next: ClipViewItem) => applied.push(next),
    }),
  });
  app.mount(root);

  try {
    const card = all(root, (element) => element.tag === "article" && hasClass(element, "clip-card-type-file"))[0];
    assert.ok(card, "file card renders with its file type");
    const title = all(card, (element) =>
      nodeText(element) === "合同 Final.PDF"
        && element.props.title === fileReference
        && element.props["data-tooltip"] === fileReference,
    )[0];
    assert.ok(title, "file card exposes its filename with the complete reference as a tooltip");
    assert.equal(nodeText(card).includes(fileReference), false, "the full path is not laid out as card body text");

    const metric = all(card, (element) => hasClass(element, "clip-metric-badge"))[0];
    assert.equal(nodeText(metric), "PDF");
    assert.equal(nodeText(metric).includes("stats.chars"), false, "file references do not use text character statistics");

    const doubleClick = card.props.onDblclick as ((input: ReturnType<typeof event>) => void) | undefined;
    assert.ok(doubleClick, "file card retains its apply gesture");
    doubleClick(event(card));
    assert.deepEqual(applied, [item]);
    assert.equal(applied[0].text, fileReference);
  } finally {
    app.unmount();
  }
});

test("the file viewer is read-only and applies the original file reference", async () => {
  const item = historyFile();
  const label = "file-reference-viewer";
  state.storage.clear();
  state.storage.set(`file-reference:${label}`, JSON.stringify({
    label,
    originalClipId: "source-file-1",
    item,
  }));
  state.viewerApi = {
    applyClip: async (...args: unknown[]) => {
      state.invokeCalls.push({ command: "viewer-apply", args: { values: args } });
    },
    closeClipViewer: async () => {},
    updateClipContent: async () => {
      throw new Error("file viewer must not save editable content");
    },
  };
  globalThis.window.location.search = `?label=${label}`;

  const root = new HostNode("root");
  const app = renderer.createApp(ClipViewerWindow);
  app.mount(root);
  await nextTick();

  try {
    const editor = all(root, (element) => element.tag === "textarea" && hasClass(element, "viewer-editor"))[0];
    assert.ok(editor, "file references use the viewer content field");
    assert.equal(editor.props.value, fileReference);
    assert.notEqual(editor.props.readonly, undefined, "the path cannot be edited");
    assert.equal(editor.props["aria-label"], "file.path");

    const editableToolbarButtons = all(root, (element) =>
      element.tag === "button" && (nodeText(element).includes("viewer.reset") || nodeText(element).includes("viewer.applyChanges")),
    );
    assert.equal(editableToolbarButtons.length, 0, "file references have no reset or save-content controls");
    const footer = all(root, (element) => hasClass(element, "clip-viewer-footer"))[0];
    assert.equal(nodeText(footer).includes("stats.chars"), false);
    assert.equal(nodeText(footer).includes("common.lineCount"), false);

    const paste = all(root, (element) => element.tag === "button" && element.props["aria-label"] === "file.pasteFile")[0];
    assert.ok(paste, "file viewer exposes a file paste action");
    const click = paste.props.onClick as ((input: ReturnType<typeof event>) => Promise<void>) | undefined;
    assert.ok(click);
    await click(event(paste));
    assert.deepEqual(state.invokeCalls.at(-1), {
      command: "viewer-apply",
      args: { values: ["source-file-1", "file", fileReference] },
    });
  } finally {
    app.unmount();
  }
});

test("a failed file apply remains visible in the viewer", async () => {
  const item = historyFile();
  const label = "file-reference-error";
  const failedApplyCalls: unknown[][] = [];
  state.storage.clear();
  state.storage.set(`file-reference:${label}`, JSON.stringify({
    label,
    originalClipId: "source-file-2",
    item,
  }));
  state.viewerApi = {
    applyClip: async (...args: unknown[]) => {
      failedApplyCalls.push(args);
      throw "file.error.notFound";
    },
    closeClipViewer: async () => {},
    updateClipContent: async () => {
      throw new Error("file viewer must not save editable content");
    },
  };
  globalThis.window.location.search = `?label=${label}`;

  const root = new HostNode("root");
  const app = renderer.createApp(ClipViewerWindow);
  app.mount(root);
  await nextTick();

  try {
    const paste = all(root, (element) => element.tag === "button" && element.props["aria-label"] === "file.pasteFile")[0];
    assert.ok(paste);
    const click = paste.props.onClick as ((input: ReturnType<typeof event>) => Promise<void>) | undefined;
    assert.ok(click);
    await click(event(paste));
    await nextTick();
    assert.deepEqual(failedApplyCalls, [["source-file-2", "file", fileReference]]);
    const failure = all(root, (element) => hasClass(element, "viewer-error"))[0];
    assert.ok(failure, "the viewer keeps an actionable error visible after the native file apply fails");
    assert.equal(nodeText(failure), "file.error.notFound");
  } finally {
    app.unmount();
  }
});

test("file API calls use native file commands while copy-path stays an exact text copy", async () => {
  state.invokeCalls.length = 0;
  await nativeFileApi.copyClip("file", fileReference);
  await nativeFileApi.copyTextEphemeral(fileReference);
  await nativeFileApi.applyTextEphemeral(fileReference);
  await nativeFileApi.fileReferencePreview(fileReference);
  assert.deepEqual(state.invokeCalls, [
    { command: "copy_clip", args: { clipType: "file", text: fileReference } },
    { command: "copy_text_ephemeral", args: { text: fileReference } },
    { command: "apply_text_ephemeral", args: { text: fileReference } },
    { command: "file_reference_preview", args: { path: fileReference } },
  ]);

  state.clipboardWrites.length = 0;
  assert.equal(await browserFileApi.fileReferencePreview(fileReference), null);
  await assert.rejects(
    () => browserFileApi.copyClip("file", fileReference),
    (error) => error === "file.error.nativeOnly",
  );
  assert.deepEqual(state.clipboardWrites, []);
  await browserFileApi.copyTextEphemeral(fileReference);
  assert.deepEqual(state.clipboardWrites, [fileReference]);
});

test("stable native file error codes map without parsing platform error text", () => {
  const cases = [
    ["FILE_NOT_FOUND", "file.error.notFound"],
    ["FILE_READ_ONLY", "file.error.readOnly"],
    ["FILE_NATIVE_ONLY", "file.error.nativeOnly"],
  ] as const;
  for (const [code, message] of cases) {
    assert.throws(() => localizeFileError(code), (error) => error === message, code);
  }

  const unknownError = new Error("not a stable file error code");
  assert.throws(() => localizeFileError(unknownError), (error) => error === unknownError);
});
