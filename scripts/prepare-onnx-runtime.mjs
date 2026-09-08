import { createHash } from "node:crypto";
import { createWriteStream } from "node:fs";
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const runtimeVersion = "1.29.0";
const archiveName = `onnxruntime-win-x64-${runtimeVersion}.zip`;
const archiveUrl = `https://github.com/microsoft/onnxruntime/releases/download/v${runtimeVersion}/${archiveName}`;
const archiveRoot = `onnxruntime-win-x64-${runtimeVersion}`;
const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, "..");
const runtimeDir = path.join(projectRoot, "src-tauri", "runtime", "windows-x64");
const runtimeBinaries = [
  {
    name: "onnxruntime.dll",
    source: "lib/onnxruntime.dll",
    size: 16_149_344,
    sha256: "69d8e6d3879a3b4001cdc74c8ed9ccc7e7f799a5b847059738323404519ec471",
  },
  {
    name: "onnxruntime_providers_shared.dll",
    source: "lib/onnxruntime_providers_shared.dll",
    size: 21_856,
    sha256: "87f6878cdc1f80b3a9afa5b0c84663315030b4957f5bbb6b66470557cb2f48d8",
  },
];
const runtimeNotices = [
  {
    name: "ONNX_RUNTIME_LICENSE.txt",
    source: "LICENSE",
    url: `https://raw.githubusercontent.com/microsoft/onnxruntime/v${runtimeVersion}/LICENSE`,
    size: 1_073,
    sha256: "2f07c72751aed99790b8a4869cf2311df85a860b22ded05fa22803587a48922c",
  },
  {
    name: "ONNX_RUNTIME_THIRD_PARTY_NOTICES.txt",
    source: "ThirdPartyNotices.txt",
    url: `https://raw.githubusercontent.com/microsoft/onnxruntime/v${runtimeVersion}/ThirdPartyNotices.txt`,
    size: 336_906,
    sha256: "53d3fa5821ac016ac24dd35775c996efec86e2ae0841e9a3a5e146c0ae916845",
  },
];
const runtimeFiles = [...runtimeBinaries, ...runtimeNotices];

if (process.platform !== "win32") {
  console.log("ONNX Runtime preparation is only required for Windows builds; skipping.");
  process.exit(0);
}

if (await filesAreReady(runtimeBinaries)) {
  await prepareRuntimeNotices();
}

if (await filesAreReady(runtimeFiles)) {
  console.log(`ONNX Runtime ${runtimeVersion} is ready.`);
  process.exit(0);
}

const tempRoot = await mkdtemp(path.join(tmpdir(), "ipaste-onnx-runtime-"));
const archivePath = path.join(tempRoot, archiveName);
const extractDir = path.join(tempRoot, "extract");

try {
  console.log(`Downloading ONNX Runtime ${runtimeVersion}...`);
  await downloadToFile(archiveUrl, archivePath);
  await mkdir(extractDir, { recursive: true });

  const archiveEntries = runtimeBinaries.map(({ source }) => `${archiveRoot}/${source}`);
  execFileSync("tar", ["-xf", archivePath, "-C", extractDir, ...archiveEntries], {
    stdio: "inherit",
    windowsHide: true,
  });

  await mkdir(runtimeDir, { recursive: true });
  for (const file of runtimeBinaries) {
    const extractedPath = path.join(extractDir, archiveRoot, ...file.source.split("/"));
    await verifyFile(extractedPath, file);
    await copyFile(extractedPath, path.join(runtimeDir, file.name));
  }
} finally {
  await rm(tempRoot, { recursive: true, force: true });
}

await prepareRuntimeNotices();
if (!(await filesAreReady(runtimeFiles))) {
  throw new Error(`ONNX Runtime ${runtimeVersion} preparation did not complete.`);
}
console.log(`Prepared ONNX Runtime ${runtimeVersion} for the Windows bundle.`);

async function filesAreReady(files) {
  try {
    for (const file of files) {
      await verifyFile(path.join(runtimeDir, file.name), file);
    }
    return true;
  } catch {
    return false;
  }
}

async function prepareRuntimeNotices() {
  await mkdir(runtimeDir, { recursive: true });
  for (const file of runtimeNotices) {
    const targetPath = path.join(runtimeDir, file.name);
    if (await filesAreReady([file])) continue;

    const response = await fetchWithRetry(file.url);
    const content = Buffer.from(await response.arrayBuffer());
    await writeFile(targetPath, content);
    await verifyFile(targetPath, file);
  }
}

async function downloadToFile(url, targetPath) {
  let lastError;
  for (let attempt = 1; attempt <= 3; attempt += 1) {
    try {
      const response = await fetchWithRetry(url, 1);
      if (!response.body) throw new Error(`Empty response body from ${url}`);
      await pipeline(Readable.fromWeb(response.body), createWriteStream(targetPath));
      return;
    } catch (error) {
      lastError = error;
      if (attempt < 3) await delay(attempt * 1_000);
    }
  }
  throw lastError;
}

async function fetchWithRetry(url, attempts = 3) {
  let lastError;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      const response = await fetch(url, { redirect: "follow" });
      if (!response.ok) throw new Error(`HTTP ${response.status} from ${url}`);
      return response;
    } catch (error) {
      lastError = error;
      if (attempt < attempts) await delay(attempt * 1_000);
    }
  }
  throw lastError;
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function verifyFile(filePath, expected) {
  const content = await readFile(filePath);
  if (content.byteLength !== expected.size) {
    throw new Error(`${expected.name} has an unexpected size.`);
  }
  const actualSha256 = createHash("sha256").update(content).digest("hex");
  if (actualSha256 !== expected.sha256) {
    throw new Error(`${expected.name} failed SHA-256 verification.`);
  }
}
