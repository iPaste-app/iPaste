import { build } from "esbuild";
import { createRequire } from "node:module";
import { runInThisContext } from "node:vm";

const result = await build({
  entryPoints: ["tests/mfa.test.ts"], bundle: true, write: false,
  platform: "node", format: "cjs",
});
const execute = runInThisContext(`(function(require, module, exports) {\n${result.outputFiles[0].text}\n})`, {
  filename: "mfa-regression.cjs",
});
const testModule = { exports: {} };
execute(createRequire(import.meta.url), testModule, testModule.exports);
