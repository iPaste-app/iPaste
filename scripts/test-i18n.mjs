import { parse, compileScript } from "@vue/compiler-sfc";
import { build } from "esbuild";
import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { runInThisContext } from "node:vm";

const storage = new Map([["ipaste.language", "zh-TW"]]);
Object.defineProperty(globalThis, "localStorage", {
  configurable: true,
  value: { getItem: (key) => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value) },
});
globalThis.document = {
  documentElement: {},
  createElement: () => ({}),
  addEventListener() {},
  removeEventListener() {},
};

const { outputFiles } = await build({
  entryPoints: ["tests/i18n.test.ts"],
  bundle: true,
  write: false,
  platform: "node",
  format: "cjs",
  plugins: [{
    name: "compile-language-select",
    setup(builder) {
      builder.onLoad({ filter: /LanguageSelect\.vue$/ }, async ({ path: filename }) => {
        const source = await readFile(filename, "utf8");
        // Exercise real component state and callbacks without a DOM or CSS transitions.
        const { descriptor } = parse(source.replace("<Transition ", '<Transition :css="false" '), { filename });
        const { content } = compileScript(descriptor, { id: "language-select-test", inlineTemplate: true });
        return { contents: content, loader: "ts" };
      });
    },
  }],
});
const execute = runInThisContext(`(function(require, module, exports) {\n${outputFiles[0].text}\n})`, { filename: "i18n-regression.cjs" });
const testModule = { exports: {} };
execute(createRequire(import.meta.url), testModule, testModule.exports);
