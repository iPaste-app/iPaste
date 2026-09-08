import { build } from "esbuild";
import { parse, compileScript } from "@vue/compiler-sfc";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { createRequire } from "node:module";
import { runInThisContext } from "node:vm";

// Exercise the actual settings template without launching a browser or native window.
globalThis.window = { __TAURI_INTERNALS__: {}, location: { search: "" }, addEventListener() {}, removeEventListener() {} };
globalThis.document = { activeElement: null, createElement() { return {}; }, addEventListener() {}, removeEventListener() {} };
Object.defineProperty(globalThis, "navigator", { configurable: true, value: { platform: "Win32", userAgent: "Windows" } });
globalThis.__settingsTest = {};

const stubs = {
  store: "export const useIpasteStore = () => globalThis.__settingsTest.store;",
  api: "export const ipasteApi = new Proxy({}, {get: (_, key) => (...args) => globalThis.__settingsTest.api[key](...args)});",
  autostart: "export const isEnabled = () => globalThis.__settingsTest.readAutostart(); export const enable = () => {globalThis.__settingsTest.writes++}; export const disable = enable;",
  i18n: "export const t = key => key; export const languageOptions = [];",
  format: "export const formatShortcut = key => key;",
  star: "export const IPASTE_GITHUB_REPOSITORY_URL = 'https://github.com/iPaste-app/iPaste'; export const openGitHubRepository = async () => {};",
  updater: "import {ref} from 'vue'; export const useUpdater = () => new Proxy({}, {get: (_, key) => key.startsWith('open') || key.startsWith('dismiss') ? () => {} : ref(null)});",
  child: "export default {render: () => null};",
  opener: "export const openPath = async path => {globalThis.__settingsTest.openedPaths.push(path)}; export const openUrl = async url => {globalThis.__settingsTest.openedUrls.push(url)};",
  event: "export const listen = (...args) => globalThis.__settingsTest.listen(...args);",
  ocr: `
    import {onMounted} from 'vue';
    export const OCR_MODELS = [
      {mode: 'fast', name: 'PP-OCRv6 Tiny', totalBytes: 6932119},
      {mode: 'best', name: 'PP-OCRv6 Small', totalBytes: 31824456},
    ];
    export const formatOcrBytes = bytes => (bytes / 1000000).toFixed(1) + ' MB';
    export const useOcrModels = () => {
      onMounted(() => globalThis.__settingsTest.ocrMounts++);
      return globalThis.__settingsTest.ocr;
    };
  `,
};
const components = new Map(await Promise.all(["SettingsWindow.vue", "OcrSettingsPanel.vue", "OcrSetupGuide.vue"].map(async name => {
  const filename = path.resolve("src/components", name);
  return [filename, await readFile(filename, "utf8")];
})));
const icons = new Set([...components.values()].flatMap(source =>
  source.match(/import\s*{([^}]+)}\s*from "lucide-vue-next"/s)[1].split(",").map(x => x.trim()).filter(Boolean),
));
stubs.icons = [...icons].map(name => `export const ${name} = {render: () => null};`).join("\n");
const routes = [
  [/stores\/ipasteStore$/, "store"], [/lib\/ipasteApi$/, "api"],
  [/@tauri-apps\/plugin-autostart$/, "autostart"], [/\/i18n$/, "i18n"],
  [/lib\/format$/, "format"], [/lib\/starPrompt$/, "star"],
  [/composables\/useUpdater$/, "updater"], [/^lucide-vue-next$/, "icons"],
  [/composables\/useOcrModels$/, "ocr"], [/@tauri-apps\/plugin-opener$/, "opener"],
  [/@tauri-apps\/api\/event$/, "event"],
];
const result = await build({
  entryPoints: ["tests/settingsWindow.test.ts"], bundle: true, write: false,
  platform: "node", format: "cjs",
  plugins: [{ name: "settings-test", setup(builder) {
    builder.onResolve({ filter: /.*/ }, ({ path: source }) => {
      const route = routes.find(([pattern]) => pattern.test(source));
      if (route) return { path: route[1], namespace: "settings-test" };
      if (source.endsWith(".vue") && ![...components.keys()].some(filename => path.basename(filename) === path.basename(source))) {
        return { path: "child", namespace: "settings-test" };
      }
    });
    builder.onLoad({ filter: /.*/, namespace: "settings-test" }, ({ path: name }) => ({ contents: stubs[name], resolveDir: process.cwd() }));
    builder.onLoad({ filter: /\.vue$/ }, ({ path: filename }) => {
      const { descriptor } = parse(components.get(filename), { filename });
      const compiled = compileScript(descriptor, {
        id: path.basename(filename, ".vue"), inlineTemplate: true,
        templateOptions: { compilerOptions: { hoistStatic: false } },
      });
      return { contents: compiled.content, loader: "ts", resolveDir: path.dirname(filename) };
    });
  } }],
});
// A named in-memory bundle keeps failure locations readable without writing build artifacts.
const execute = runInThisContext(`(function(require, module, exports) {\n${result.outputFiles[0].text}\n})`, {
  filename: "settings-window-regression.cjs",
});
const testModule = { exports: {} };
execute(createRequire(import.meta.url), testModule, testModule.exports);
