import { build } from "esbuild";
import { createRequire } from "node:module";
import { runInThisContext } from "node:vm";

// A DOM-free Vue renderer tests transitions without opening a browser or touching models.
globalThis.window = { __TAURI_INTERNALS__: {} };
globalThis.__ocrTest = {};
const stubs = {
  "ipasteApi": "export const ipasteApi = new Proxy({}, {get: (_, key) => (...args) => globalThis.__ocrTest.api[key](...args)});",
  "ipasteStore": "export const useIpasteStore = () => globalThis.__ocrTest.store;",
  "event": "export const listen = (...args) => globalThis.__ocrTest.listen(...args);",
};
const result = await build({
  entryPoints: ["tests/ocrModels.test.ts"], bundle: true, write: false,
  platform: "node", format: "cjs",
  plugins: [{
    name: "desktop-test-stubs",
    setup(builder) {
      builder.onResolve({ filter: /(?:lib\/ipasteApi|stores\/ipasteStore|@tauri-apps\/api\/event)$/ }, ({ path }) => ({ path: path.split("/").at(-1), namespace: "ocr-test" }));
      builder.onLoad({ filter: /.*/, namespace: "ocr-test" }, ({ path }) => ({ contents: stubs[path] }));
    },
  }],
});
const execute = runInThisContext(`(function(require, module, exports) {\n${result.outputFiles[0].text}\n})`, {
  filename: "ocr-models-regression.cjs",
});
const testModule = { exports: {} };
execute(createRequire(import.meta.url), testModule, testModule.exports);
