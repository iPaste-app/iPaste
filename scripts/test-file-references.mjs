import { build } from "esbuild";
import { parse, compileScript } from "@vue/compiler-sfc";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { createRequire } from "node:module";
import { runInThisContext } from "node:vm";

const root = process.cwd();
const componentFiles = [
  path.resolve(root, "src/components/ClipCard.vue"),
  path.resolve(root, "src/components/ClipViewerWindow.vue"),
];

const state = {
  clipboardWrites: [],
  invokeCalls: [],
  storage: new Map(),
  viewerApi: {},
};
globalThis.__fileReferencesTest = state;
globalThis.window = {
  location: { search: "" },
  innerHeight: 720,
  innerWidth: 960,
  addEventListener() {},
  removeEventListener() {},
  clearInterval,
  clearTimeout,
  getSelection: () => null,
  setInterval,
  setTimeout,
};
globalThis.document = {
  activeElement: null,
  hidden: false,
  addEventListener() {},
  removeEventListener() {},
  body: { appendChild() {}, removeChild() {} },
  createElement() { return { append() {}, style: { setProperty() {} } }; },
  createTextNode(text) { return { textContent: text }; },
  querySelector() { return null; },
};
globalThis.localStorage = {
  getItem: (key) => state.storage.get(key) ?? null,
  removeItem: (key) => state.storage.delete(key),
  setItem: (key, value) => state.storage.set(key, value),
};
globalThis.ResizeObserver = class { observe() {} disconnect() {} };
Object.defineProperty(globalThis, "navigator", {
  configurable: true,
  value: {
    platform: "Win32",
    userAgent: "Windows",
    clipboard: { writeText: async (text) => { state.clipboardWrites.push(text); } },
  },
});

const stubs = {
  api: `
    export const clipViewerStorageKey = label => "file-reference:" + label;
    export const ipasteApi = new Proxy({}, {
      get: (_, key) => (...args) => globalThis.__fileReferencesTest.viewerApi[key]?.(...args),
    });
  `,
  child: "export default { render: () => null };",
  core: "export const invoke = (command, args) => { globalThis.__fileReferencesTest.invokeCalls.push({ command, args }); return Promise.resolve(undefined); }; export const convertFileSrc = value => value;",
  event: "export const emit = async () => {};",
  i18n: "export type I18nKey = string; export const currentLocale = { value: 'en' }; export const t = (key) => key;",
  media: "export const clipImageSrc = () => '';",
  window: "export const getCurrentWindow = () => ({ hide: async () => {}, show: async () => {}, setAlwaysOnTop: async () => {}, setFocus: async () => {}, isAlwaysOnTop: async () => false, onCloseRequested: async () => () => {}, startDragging: async () => {} });",
};

const targetComponents = new Set(componentFiles);
const apiSource = await readFile(path.resolve(root, "src/lib/ipasteApi.ts"), "utf8");
const result = await build({
  entryPoints: ["tests/fileReferences.test.ts"],
  bundle: true,
  write: false,
  platform: "node",
  format: "cjs",
  plugins: [{
    name: "file-reference-test",
    setup(builder) {
      builder.onResolve({ filter: /^@file-test\/api-(native|browser)$/ }, ({ path: source }) => ({
        path: source.endsWith("native") ? "native" : "browser",
        namespace: "file-reference-api",
      }));
      builder.onResolve({ filter: /^@tauri-apps\/api\/core$/ }, () => ({ path: "core", namespace: "file-reference-test" }));
      builder.onResolve({ filter: /^@tauri-apps\/api\/event$/ }, () => ({ path: "event", namespace: "file-reference-test" }));
      builder.onResolve({ filter: /^@tauri-apps\/api\/window$/ }, () => ({ path: "window", namespace: "file-reference-test" }));
      builder.onResolve({ filter: /(?:^|\/)lib\/ipasteApi$/ }, () => ({ path: "api", namespace: "file-reference-test" }));
      builder.onResolve({ filter: /(?:^|\/)lib\/clipMedia$/ }, () => ({ path: "media", namespace: "file-reference-test" }));
      builder.onResolve({ filter: /(?:^|\/)i18n$/ }, () => ({ path: "i18n", namespace: "file-reference-test" }));
      builder.onResolve({ filter: /\.vue$/ }, ({ path: source, resolveDir }) => {
        const filename = path.resolve(resolveDir, source);
        if (targetComponents.has(filename)) return { path: filename };
        return { path: "child", namespace: "file-reference-test" };
      });
      builder.onLoad({ filter: /.*/, namespace: "file-reference-api" }, ({ path: mode }) => {
        const setupWindow = mode === "native"
          ? "globalThis.window = { ...globalThis.window, __TAURI_INTERNALS__: {} };"
          : "const { __TAURI_INTERNALS__, ...browserWindow } = globalThis.window; globalThis.window = browserWindow;";
        return {
          contents: `${setupWindow}\n${apiSource}`,
          loader: "ts",
          resolveDir: path.resolve(root, "src/lib"),
        };
      });
      builder.onLoad({ filter: /.*/, namespace: "file-reference-test" }, ({ path: name }) => ({
        contents: stubs[name],
        loader: "ts",
        resolveDir: root,
      }));
      builder.onLoad({ filter: /\.vue$/ }, async ({ path: filename }) => {
        const { descriptor } = parse(await readFile(filename, "utf8"), { filename });
        const compiled = compileScript(descriptor, {
          id: path.basename(filename, ".vue"),
          inlineTemplate: true,
          templateOptions: { compilerOptions: { hoistStatic: false } },
        });
        return { contents: compiled.content, loader: "ts", resolveDir: path.dirname(filename) };
      });
    },
  }],
});

const execute = runInThisContext(`(function(require, module, exports) {\n${result.outputFiles[0].text}\n})`, {
  filename: "file-references-regression.cjs",
});
const testModule = { exports: {} };
execute(createRequire(import.meta.url), testModule, testModule.exports);
