import { build } from "esbuild";
import { createRequire } from "node:module";
import { runInThisContext } from "node:vm";

const root = process.cwd();

const state = {
  requests: [],
  convertedPaths: [],
  imageThumbnail(path) {
    let resolve;
    let reject;
    const promise = new Promise((nextResolve, nextReject) => {
      resolve = nextResolve;
      reject = nextReject;
    });
    this.requests.push({ path, resolve, reject });
    return promise;
  },
  convertFileSrc(path) {
    this.convertedPaths.push(path);
    return `asset://${path}`;
  },
};

globalThis.__imageThumbnailTest = state;
globalThis.window = { __TAURI_INTERNALS__: {} };

const result = await build({
  entryPoints: ["tests/imageThumbnails.test.ts"],
  bundle: true,
  write: false,
  platform: "node",
  format: "cjs",
  plugins: [{
    name: "image-thumbnail-test",
    setup(builder) {
      builder.onResolve({ filter: /^@tauri-apps\/api\/core$/ }, () => ({
        path: "core",
        namespace: "image-thumbnail-test",
      }));
      builder.onResolve({ filter: /(?:^|\/)lib\/ipasteApi$/ }, () => ({
        path: "api",
        namespace: "image-thumbnail-test",
      }));
      builder.onLoad({ filter: /.*/, namespace: "image-thumbnail-test" }, ({ path }) => ({
        contents: path === "core"
          ? "export const convertFileSrc = path => globalThis.__imageThumbnailTest.convertFileSrc(path);"
          : "export const ipasteApi = { imageThumbnail: path => globalThis.__imageThumbnailTest.imageThumbnail(path) };",
        loader: "ts",
        resolveDir: root,
      }));
    },
  }],
});

const execute = runInThisContext(`(function(require, module, exports) {\n${result.outputFiles[0].text}\n})`, {
  filename: "image-thumbnails-regression.cjs",
});
const testModule = { exports: {} };
execute(createRequire(import.meta.url), testModule, testModule.exports);
