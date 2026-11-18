import { build } from "esbuild";
import { createRequire } from "node:module";
import { runInThisContext } from "node:vm";

globalThis.window = { __TAURI_INTERNALS__: {} };
globalThis.__lanShareTest = { calls: [], handlers: {}, copied: [] };
const result = await build({
  entryPoints: ["tests/lanShare.test.ts"],
  bundle: true,
  write: false,
  platform: "node",
  format: "cjs",
  plugins: [{
    name: "lan-share-native-stubs",
    setup(builder) {
      builder.onResolve({ filter: /^@tauri-apps\/api\/core$/ }, () => ({ path: "native", namespace: "share-test" }));
      builder.onResolve({ filter: /(?:^|\/)lib\/ipasteApi$/ }, () => ({ path: "clipboard", namespace: "share-test" }));
      builder.onResolve({ filter: /(?:^|\/)i18n$/ }, () => ({ path: "i18n", namespace: "share-test" }));
      builder.onLoad({ filter: /.*/, namespace: "share-test" }, ({ path }) => ({
        loader: "ts",
        contents: {
          native: `export const invoke = (command, args) => { const state = globalThis.__lanShareTest; state.calls.push({ command, args }); return state.handlers[command]?.(args) ?? Promise.resolve(null); };`,
          clipboard: `export const ipasteApi = { copyTextEphemeral: async text => { globalThis.__lanShareTest.copied.push(text); } };`,
          i18n: `export const t = key => key; export type I18nKey = string;`,
        }[path],
      }));
    },
  }],
});
const execute = runInThisContext(`(function(require, module, exports) {\n${result.outputFiles[0].text}\n})`);
const testModule = { exports: {} };
execute(createRequire(import.meta.url), testModule, testModule.exports);
