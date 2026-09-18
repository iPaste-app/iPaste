import { build } from "esbuild";
import { parse, compileScript } from "@vue/compiler-sfc";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { createRequire } from "node:module";
import { runInThisContext } from "node:vm";

globalThis.window = { __TAURI_INTERNALS__: {}, setTimeout, clearTimeout };
globalThis.__searchTest = { requests: [], pinRequests: [], reorderRequests: [], listeners: new Map(), syncCount: 0 };
const stubs = {
  api: `export const ipasteApi = { listClips(...args) {
    return new Promise((resolve, reject) => globalThis.__searchTest.requests.push({args, resolve, reject}));
  }, setClipPinned(...args) {
    return new Promise((resolve, reject) => globalThis.__searchTest.pinRequests.push({args, resolve, reject}));
  }, reorderCategoryItems(...args) {
    return new Promise((resolve, reject) => globalThis.__searchTest.reorderRequests.push({args, resolve, reject}));
  }, async syncCloudInBackground() { globalThis.__searchTest.syncCount += 1; } };`,
  i18n: `import {ref} from 'vue'; export const currentLocale = ref('en');
    export const t = (key, args) => args ? key + ':' + JSON.stringify(args) : key;
    export const cleanLanguage = value => value; export const setLanguage = () => {};`,
  event: `export const listen = async (name, callback) => {
    globalThis.__searchTest.listeners.set(name, callback);
    return () => globalThis.__searchTest.listeners.delete(name);
  };`,
};
const result = await build({
  entryPoints: [process.argv[2] ?? "tests/search.test.ts"], bundle: true, write: false,
  platform: "node", format: "cjs",
  define: {"import.meta.url": JSON.stringify(new URL("../src/components/TopBar.vue", import.meta.url).href)},
  plugins: [{ name: "search-test", setup(builder) {
    builder.onResolve({ filter: /(?:lib\/ipasteApi|\/i18n|@tauri-apps\/api\/event)$/ }, ({path: source}) => ({
      path: source.endsWith("ipasteApi") ? "api" : source.endsWith("i18n") ? "i18n" : "event",
      namespace: "search-test",
    }));
    builder.onLoad({ filter: /.*/, namespace: "search-test" }, ({path: name}) => ({
      contents: stubs[name], resolveDir: process.cwd(),
    }));
    builder.onLoad({ filter: /\.vue$/ }, async ({path: filename}) => {
      const {descriptor} = parse(await readFile(filename, "utf8"), {filename});
      const compiled = compileScript(descriptor, {
        id: path.basename(filename, ".vue"), inlineTemplate: true,
        templateOptions: {compilerOptions: {hoistStatic: false}},
      });
      return {contents: compiled.content, loader: "ts", resolveDir: path.dirname(filename)};
    });
  }}],
});
const execute = runInThisContext(`(function(require, module, exports) {\n${result.outputFiles[0].text}\n})`, {
  filename: "search-regression.cjs",
});
const testModule = {exports: {}};
execute(createRequire(import.meta.url), testModule, testModule.exports);
