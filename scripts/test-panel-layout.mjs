import { build } from "esbuild";
import { createRequire } from "node:module";
import { runInThisContext } from "node:vm";

const result = await build({
  entryPoints: ["tests/panelLayout.test.ts"], bundle: true, write: false,
  platform: "node", format: "cjs",
  plugins: [{ name: "panel-test", setup(builder) {
    builder.onResolve({ filter: /^@tauri-apps\/api\/core$/ }, () => ({ path: "invoke", namespace: "panel-test" }));
    builder.onLoad({ filter: /.*/, namespace: "panel-test" }, () => ({
      contents: "export const invoke = (...args) => globalThis.__panelInvoke(...args);",
    }));
  } }],
});
const execute = runInThisContext(`(function(require, module, exports) {\n${result.outputFiles[0].text}\n})`, {
  filename: "panel-layout-regression.cjs",
});
const testModule = { exports: {} };
execute(createRequire(import.meta.url), testModule, testModule.exports);
