import { build } from "esbuild";
import { parse, compileScript } from "@vue/compiler-sfc";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { createRequire } from "node:module";
import { runInThisContext } from "node:vm";

// Render real Vue components with a minimal host; no browser or native clipboard.
globalThis.__mfaTest = { listeners: new Map(), api: {}, store: { clips: [] } };
globalThis.ResizeObserver = class { observe() {} disconnect() {} };
globalThis.window = {
  setTimeout: (...args) => setTimeout(...args).unref(), clearTimeout,
  setInterval: (...args) => setInterval(...args).unref(), clearInterval,
  addEventListener() {}, removeEventListener() {}, getSelection: () => null,
};
globalThis.document = {
  activeElement: null, body: null, createElement() { return {}; }, querySelector: () => null,
  addEventListener(name, handler) { globalThis.__mfaTest.listeners.set(name, handler); },
  removeEventListener(name) { globalThis.__mfaTest.listeners.delete(name); },
};
const stubs = {
  store: "export const useIpasteStore = () => globalThis.__mfaTest.store;",
  api: "export const ipasteApi = new Proxy({}, {get: (_, key) => (...args) => globalThis.__mfaTest.api[key](...args)});",
  i18n: "export const t = (key, params = {}) => key + (params.seconds === undefined ? '' : ':' + params.seconds);",
  qr: "export const decodeQrFromClip = async () => null; export const decodeQrFromFile = async () => null;",
};
const routes = [[/stores\/ipasteStore$/, "store"], [/lib\/ipasteApi$/, "api"], [/\/i18n$/, "i18n"], [/lib\/qr$/, "qr"]];
const result = await build({
  entryPoints: ["tests/mfaInteractions.test.ts"], bundle: true, write: false, platform: "node", format: "cjs",
  plugins: [{ name: "mfa-test", setup(builder) {
    builder.onResolve({ filter: /.*/ }, ({ path: source }) => {
      const route = routes.find(([pattern]) => pattern.test(source));
      if (route) return { path: route[1], namespace: "mfa-test" };
    });
    builder.onLoad({ filter: /.*/, namespace: "mfa-test" }, ({ path: name }) => ({ contents: stubs[name], resolveDir: process.cwd() }));
    builder.onLoad({ filter: /\.vue$/ }, async ({ path: filename }) => {
      const { descriptor } = parse(await readFile(filename, "utf8"), { filename });
      const compiled = compileScript(descriptor, {
        id: path.basename(filename, ".vue"), inlineTemplate: true,
        templateOptions: { compilerOptions: { hoistStatic: false } },
      });
      return { contents: compiled.content, loader: "ts", resolveDir: path.dirname(filename) };
    });
  } }],
});
const execute = runInThisContext(`(function(require, module, exports) {\n${result.outputFiles[0].text}\n})`, { filename: "mfa-interactions.cjs" });
const testModule = { exports: {} };
execute(createRequire(import.meta.url), testModule, testModule.exports);
