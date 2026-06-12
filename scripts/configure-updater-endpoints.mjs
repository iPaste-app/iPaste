import { appendFile, readFile, writeFile } from "node:fs/promises";

const configPath = "src-tauri/tauri.conf.json";
const r2Endpoint = process.env.IPASTE_UPDATER_R2_ENDPOINT?.trim();
const ocrR2BaseUrl = process.env.IPASTE_OCR_R2_BASE_URL?.trim() || deriveOcrR2BaseUrl(r2Endpoint);

if (ocrR2BaseUrl) {
  console.log(`Using OCR R2 base URL: ${ocrR2BaseUrl}`);
  if (process.env.GITHUB_ENV) {
    await appendFile(process.env.GITHUB_ENV, `IPASTE_OCR_R2_BASE_URL=${ocrR2BaseUrl}\n`);
  }
}

if (!r2Endpoint) {
  console.log("IPASTE_UPDATER_R2_ENDPOINT is not set; using updater endpoints from tauri.conf.json.");
  process.exit(0);
}

const url = new URL(r2Endpoint);
if (url.protocol !== "https:") {
  throw new Error("IPASTE_UPDATER_R2_ENDPOINT must use https.");
}

const config = JSON.parse(await readFile(configPath, "utf8"));
const updater = config.plugins?.updater;

if (!updater || !Array.isArray(updater.endpoints)) {
  throw new Error(`Missing plugins.updater.endpoints in ${configPath}.`);
}

updater.endpoints = [
  r2Endpoint,
  ...updater.endpoints.filter((endpoint) => endpoint !== r2Endpoint),
];

await writeFile(configPath, `${JSON.stringify(config, null, 2)}\n`);
console.log(`Prepended R2 updater endpoint: ${r2Endpoint}`);

function deriveOcrR2BaseUrl(endpoint) {
  if (!endpoint) return "";
  const url = new URL(endpoint);
  if (url.protocol !== "https:") return "";
  url.search = "";
  url.hash = "";
  const segments = url.pathname.split("/");
  segments.pop();
  segments.push("ocr", "");
  url.pathname = segments.join("/") || "/ocr/";
  return url.toString();
}
