import { build } from "esbuild";
import { compileScript, parse } from "@vue/compiler-sfc";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { runInThisContext } from "node:vm";

// Keep Vue and the update dialog real; replace only native I/O and presentation dependencies.
globalThis.window = { __TAURI_INTERNALS__: {} };
globalThis.document = { activeElement: null, createElement() { return {}; } };
globalThis.HTMLElement = class {};
globalThis.__updaterTest = { snapshots: [] };
const stubs = {
  updater: "export const check = (...args) => globalThis.__updaterTest.check(...args);",
  process: "export const relaunch = () => globalThis.__updaterTest.relaunch();",
  window: "export const getCurrentWindow = () => ({label: 'main'});",
  event: `
    export const listen = async (_, handler) => { globalThis.__updaterTest.onCommand = handler; return () => {}; };
    export const emit = async (_, payload) => { globalThis.__updaterTest.snapshots.push(payload); };
    export const emitTo = async () => {};
  `,
  i18n: "export const t = key => key;",
  star: "export const openGitHubRelease = async () => {};",
  icons: "export const Download = {render: () => null}; export const ExternalLink = Download; export const RotateCw = Download; export const X = Download;",
  markdown: "export default {render: () => null};",
};
const routes = [
  [/^@tauri-apps\/plugin-updater$/, "updater"], [/^@tauri-apps\/plugin-process$/, "process"],
  [/^@tauri-apps\/api\/window$/, "window"], [/^@tauri-apps\/api\/event$/, "event"],
  [/\/i18n$/, "i18n"], [/\/lib\/starPrompt$/, "star"],
  [/^lucide-vue-next$/, "icons"], [/MarkdownPreview\.vue$/, "markdown"],
];
const result = await build({
  entryPoints: ["tests/updater.test.ts"], bundle: true, write: false,
  platform: "node", format: "cjs", define: { "import.meta.hot": "undefined" },
  plugins: [{ name: "updater-test", setup(builder) {
    builder.onResolve({ filter: /.*/ }, ({ path: source }) => {
      const route = routes.find(([pattern]) => pattern.test(source));
      if (route) return { path: route[1], namespace: "updater-test" };
    });
    builder.onLoad({ filter: /.*/, namespace: "updater-test" }, ({ path: name }) => ({ contents: stubs[name] }));
    builder.onLoad({ filter: /\.vue$/ }, async ({ path: filename }) => {
      const { descriptor } = parse(await readFile(filename, "utf8"), { filename });
      const compiled = compileScript(descriptor, { id: "update-dialog", inlineTemplate: true });
      return { contents: compiled.content, loader: "ts", resolveDir: path.dirname(filename) };
    });
  } }],
});
const execute = runInThisContext(`(function(require, module, exports) {\n${result.outputFiles[0].text}\n})`, {
  filename: "updater-regression.cjs",
});
const testModule = { exports: {} };
execute(createRequire(import.meta.url), testModule, testModule.exports);
