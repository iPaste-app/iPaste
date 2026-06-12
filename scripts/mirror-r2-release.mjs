import { mkdtemp, readdir, readFile, stat, writeFile } from "node:fs/promises";
import { readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { createHash } from "node:crypto";
import path from "node:path";
import { execFileSync } from "node:child_process";

const releaseDir = process.env.R2_RELEASE_DIR ?? "dist/r2-release";
const ocrAssetDir = process.env.OCR_ASSET_OUTPUT_DIR ?? "output/ocr-assets";
const manifestEndpoint = process.env.IPASTE_UPDATER_R2_ENDPOINT?.trim();
const ocrR2BaseUrl = process.env.IPASTE_OCR_R2_BASE_URL?.trim() || deriveOcrR2BaseUrl(manifestEndpoint);
const requiredEnv = [
  "R2_ACCOUNT_ID",
  "R2_BUCKET",
  "R2_ACCESS_KEY_ID",
  "R2_SECRET_ACCESS_KEY",
];

if (!manifestEndpoint) {
  console.log("IPASTE_UPDATER_R2_ENDPOINT is not set; skipping R2 mirror.");
  process.exit(0);
}

const missing = requiredEnv.filter((key) => !process.env[key]?.trim());
if (missing.length > 0) {
  throw new Error(`Missing required R2 secrets: ${missing.join(", ")}.`);
}

const endpointUrl = new URL(manifestEndpoint);
if (endpointUrl.protocol !== "https:") {
  throw new Error("IPASTE_UPDATER_R2_ENDPOINT must use https.");
}
if (endpointUrl.search || endpointUrl.hash) {
  throw new Error("IPASTE_UPDATER_R2_ENDPOINT must not include a query string or hash.");
}

const manifestKey = trimSlashes(endpointUrl.pathname) || "latest.json";
const manifestFileName = path.posix.basename(manifestKey);
if (!manifestFileName.endsWith(".json")) {
  throw new Error("IPASTE_UPDATER_R2_ENDPOINT must point to a JSON manifest file.");
}

const keyPrefix = normalizePrefix(path.posix.dirname(manifestKey));
const manifestBaseUrl = parentUrl(manifestEndpoint);
const releaseFiles = await collectFiles(releaseDir);
const latestJsonPath = selectLatestJson(releaseFiles);
const manifest = JSON.parse(await readFile(latestJsonPath, "utf8"));
const version = normalizeVersion(manifest.version ?? manifest.name);
const releaseId = `v${version}`;
const releasePrefix = joinKey(keyPrefix, "releases", releaseId);
const releaseBaseUrl = appendUrl(manifestBaseUrl, "releases", releaseId);
const ocrPrefix = ocrR2BaseUrl ? r2KeyPrefixFromUrl(ocrR2BaseUrl) : "";

const assetFiles = releaseFiles.filter((file) => path.basename(file) !== "latest.json");
const assetsByName = new Map(assetFiles.map((file) => [path.basename(file), file]));

rewriteManifestUrls(manifest, assetsByName, releaseBaseUrl);

const tempDir = await mkdtemp(path.join(tmpdir(), "ipaste-r2-"));
const patchedLatestJson = path.join(tempDir, "latest.json");
await writeFile(patchedLatestJson, `${JSON.stringify(manifest, null, 2)}\n`);

for (const file of assetFiles) {
  const key = joinKey(releasePrefix, path.basename(file));
  uploadFile(file, key, contentTypeFor(file), "public, max-age=31536000, immutable");
}

uploadFile(patchedLatestJson, joinKey(releasePrefix, "latest.json"), "application/json", "public, max-age=31536000, immutable");
uploadFile(patchedLatestJson, manifestKey, "application/json", "public, max-age=60");
if (ocrPrefix) {
  await uploadOcrAssets(ocrPrefix);
}
pruneOldReleases(keyPrefix, Number.parseInt(process.env.R2_KEEP_RELEASES ?? "3", 10));

console.log(`Mirrored iPaste ${releaseId} to R2 and updated ${manifestEndpoint}.`);

async function uploadOcrAssets(ocrPrefix) {
  if (!await isDirectory(ocrAssetDir)) {
    console.log(`OCR asset directory ${ocrAssetDir} does not exist; skipping OCR R2 upload.`);
    return;
  }

  const ocrFiles = (await collectFiles(ocrAssetDir))
    .filter((file) => path.basename(file).startsWith("ipaste-ocr-"));
  if (ocrFiles.length === 0) {
    console.log(`No OCR assets found in ${ocrAssetDir}; skipping OCR R2 upload.`);
    return;
  }

  const ocrBaseUrl = normalizedBaseUrl(ocrR2BaseUrl);
  await rewriteOcrManifestBaseUrls(ocrFiles, ocrBaseUrl);

  for (const file of ocrFiles) {
    const key = joinKey(ocrPrefix, path.basename(file));
    uploadFileIfChanged(file, key, contentTypeFor(file), cacheControlForOcrAsset(file));
  }
}

async function rewriteOcrManifestBaseUrls(files, baseUrl) {
  const manifests = files.filter((file) => path.basename(file).match(/^ipaste-ocr-windows-x64-(fast|best)\.json$/));
  for (const manifestPath of manifests) {
    const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    manifest.engine.baseUrl = baseUrl;
    await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  }
}

async function collectFiles(dir) {
  const entries = await readdir(dir);
  const files = [];

  for (const entry of entries) {
    const filePath = path.join(dir, entry);
    const entryStat = await stat(filePath);
    if (entryStat.isDirectory()) {
      files.push(...await collectFiles(filePath));
    } else if (entryStat.isFile()) {
      files.push(filePath);
    }
  }

  return files;
}

async function isDirectory(dir) {
  try {
    return (await stat(dir)).isDirectory();
  } catch {
    return false;
  }
}

function selectLatestJson(files) {
  const candidates = files
    .filter((file) => path.basename(file) === "latest.json")
    .sort((a, b) => a.length - b.length);

  if (candidates.length === 0) {
    throw new Error(`No latest.json found in ${releaseDir}.`);
  }

  return candidates[0];
}

function normalizeVersion(value) {
  const version = String(value ?? "").trim().replace(/^v/, "");
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(`Invalid release version in latest.json: ${value}`);
  }
  return version;
}

function rewriteManifestUrls(manifest, assetsByName, releaseBaseUrl) {
  if (manifest.platforms && typeof manifest.platforms === "object") {
    for (const platform of Object.values(manifest.platforms)) {
      rewritePlatformUrl(platform, assetsByName, releaseBaseUrl);
    }
    return;
  }

  rewritePlatformUrl(manifest, assetsByName, releaseBaseUrl);
}

function rewritePlatformUrl(platform, assetsByName, releaseBaseUrl) {
  if (!platform?.url) {
    throw new Error("latest.json is missing an updater package URL.");
  }

  const filename = decodeURIComponent(path.posix.basename(new URL(platform.url).pathname));
  if (!assetsByName.has(filename)) {
    throw new Error(`latest.json references ${filename}, but it was not found in ${releaseDir}.`);
  }

  platform.url = appendUrl(releaseBaseUrl, filename);
}

function uploadFile(file, key, contentType, cacheControl) {
  aws([
    "s3",
    "cp",
    file,
    `s3://${process.env.R2_BUCKET}/${key}`,
    "--content-type",
    contentType,
    "--cache-control",
    cacheControl,
  ]);
}

function uploadFileIfChanged(file, key, contentType, cacheControl) {
  const localHash = fileSha256(file);
  const remoteHash = remoteSha256(key);
  if (remoteHash && remoteHash === localHash) {
    console.log(`Skipped unchanged R2 asset: ${key}`);
    return;
  }

  aws([
    "s3",
    "cp",
    file,
    `s3://${process.env.R2_BUCKET}/${key}`,
    "--content-type",
    contentType,
    "--cache-control",
    cacheControl,
    "--metadata",
    `sha256=${localHash}`,
  ]);
}

function remoteSha256(key) {
  try {
    const output = aws([
      "s3api",
      "head-object",
      "--bucket",
      process.env.R2_BUCKET,
      "--key",
      key,
      "--query",
      "Metadata.sha256",
      "--output",
      "text",
    ], { encoding: "utf8", stdio: ["inherit", "pipe", "ignore"] });
    const value = String(output ?? "").trim();
    return value && value !== "None" ? value : null;
  } catch {
    return null;
  }
}

function pruneOldReleases(keyPrefix, keepReleases) {
  if (!Number.isInteger(keepReleases) || keepReleases < 1) {
    throw new Error("R2_KEEP_RELEASES must be a positive integer.");
  }

  const releasesPrefix = joinKey(keyPrefix, "releases");
  const output = aws([
    "s3api",
    "list-objects-v2",
    "--bucket",
    process.env.R2_BUCKET,
    "--prefix",
    `${releasesPrefix}/`,
    "--delimiter",
    "/",
    "--query",
    "CommonPrefixes[].Prefix",
    "--output",
    "json",
  ], { encoding: "utf8" });

  const listedPrefixes = parseAwsJsonList(output);
  const prefixes = listedPrefixes
    .map((prefix) => ({ prefix, version: parseVersionFromPrefix(prefix) }))
    .filter((release) => release.version)
    .sort((a, b) => compareVersions(b.version, a.version));

  for (const release of prefixes.slice(keepReleases)) {
    aws(["s3", "rm", `s3://${process.env.R2_BUCKET}/${release.prefix}`, "--recursive"]);
  }
}

function parseAwsJsonList(output) {
  const trimmed = String(output ?? "").trim();
  if (!trimmed || trimmed === "null") return [];

  const parsed = JSON.parse(trimmed);
  if (!Array.isArray(parsed)) {
    throw new Error("Expected an array from aws s3api list-objects-v2.");
  }

  return parsed;
}

function parseVersionFromPrefix(prefix) {
  const name = prefix.split("/").filter(Boolean).at(-1)?.replace(/^v/, "");
  const match = name?.match(/^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/);
  if (!match) return null;

  return {
    major: Number.parseInt(match[1], 10),
    minor: Number.parseInt(match[2], 10),
    patch: Number.parseInt(match[3], 10),
    prerelease: match[4] ?? "",
  };
}

function compareVersions(left, right) {
  for (const key of ["major", "minor", "patch"]) {
    if (left[key] !== right[key]) return left[key] - right[key];
  }
  if (!left.prerelease && right.prerelease) return 1;
  if (left.prerelease && !right.prerelease) return -1;
  return left.prerelease.localeCompare(right.prerelease);
}

function aws(args, options = {}) {
  const endpoint = `https://${process.env.R2_ACCOUNT_ID}.r2.cloudflarestorage.com`;
  return execFileSync("aws", ["--endpoint-url", endpoint, ...args], {
    stdio: options.encoding ? ["inherit", "pipe", "inherit"] : "inherit",
    env: {
      ...process.env,
      AWS_ACCESS_KEY_ID: process.env.R2_ACCESS_KEY_ID,
      AWS_SECRET_ACCESS_KEY: process.env.R2_SECRET_ACCESS_KEY,
      AWS_EC2_METADATA_DISABLED: "true",
    },
    ...options,
  });
}

function contentTypeFor(file) {
  if (file.endsWith(".json")) return "application/json";
  if (file.endsWith(".zip")) return "application/zip";
  if (file.endsWith(".traineddata")) return "application/octet-stream";
  if (file.endsWith(".tar.gz") || file.endsWith(".gz")) return "application/gzip";
  if (file.endsWith(".dmg")) return "application/x-apple-diskimage";
  if (file.endsWith(".msi")) return "application/octet-stream";
  if (file.endsWith(".exe")) return "application/vnd.microsoft.portable-executable";
  if (file.endsWith(".sig")) return "text/plain";
  return "application/octet-stream";
}

function cacheControlForOcrAsset(file) {
  return file.endsWith(".json")
    ? "public, max-age=60"
    : "public, max-age=31536000, immutable";
}

function parentUrl(url) {
  const parsed = new URL(url);
  const segments = parsed.pathname.split("/");
  segments.pop();
  parsed.pathname = segments.join("/") || "/";
  return parsed.toString().replace(/\/$/, "");
}

function appendUrl(baseUrl, ...segments) {
  const base = baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`;
  return `${base}${segments.map((segment) => encodeURIComponent(segment)).join("/")}`;
}

function normalizedBaseUrl(url) {
  return url.endsWith("/") ? url : `${url}/`;
}

function r2KeyPrefixFromUrl(url) {
  const parsed = new URL(url);
  if (parsed.protocol !== "https:") {
    throw new Error("IPASTE_OCR_R2_BASE_URL must use https.");
  }
  if (parsed.search || parsed.hash) {
    throw new Error("IPASTE_OCR_R2_BASE_URL must not include a query string or hash.");
  }
  return trimSlashes(parsed.pathname) || "ocr";
}

function deriveOcrR2BaseUrl(endpoint) {
  if (!endpoint) return "";
  const parsed = new URL(endpoint);
  if (parsed.protocol !== "https:") return "";
  parsed.search = "";
  parsed.hash = "";
  const segments = parsed.pathname.split("/");
  segments.pop();
  segments.push("ocr", "");
  parsed.pathname = segments.join("/") || "/ocr/";
  return parsed.toString();
}

function fileSha256(file) {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}

function joinKey(...parts) {
  return parts
    .filter((part) => part && part !== ".")
    .map((part) => trimSlashes(part))
    .filter(Boolean)
    .join("/");
}

function normalizePrefix(prefix) {
  return prefix === "." ? "" : trimSlashes(prefix);
}

function trimSlashes(value) {
  return String(value).replace(/^\/+|\/+$/g, "");
}
