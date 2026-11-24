const extensionGroups = [
  ["document", ["doc", "docm", "docx", "dot", "dotm", "dotx", "odt", "pages", "rtf", "tex", "txt"]],
  ["pdf", ["pdf"]],
  ["ebook", ["azw", "azw3", "azw4", "epub", "fb2", "ibooks", "kfx", "mobi"]],
  ["spreadsheet", ["csv", "numbers", "ods", "tsv", "xls", "xlsb", "xlsm", "xlsx", "xlt", "xltm", "xltx"]],
  ["presentation", ["key", "odp", "pot", "potm", "potx", "pps", "ppsm", "ppsx", "ppt", "pptm", "pptx"]],
  ["database", ["accdb", "db", "db3", "dbf", "duckdb", "mdb", "sqlite", "sqlite3"]],
  ["archive", [
    "7z", "ar", "bz", "bz2", "cab", "gz", "iso", "lz", "lz4", "rar", "tar", "tbz", "tgz", "txz", "xz", "z", "zip", "zst",
  ]],
  ["android-installer", ["aab", "apk", "apkm", "apks", "xapk"]],
  ["windows-installer", ["appx", "appxbundle", "exe", "msi", "msix", "msixbundle"]],
  ["apple-installer", ["app", "dmg", "ipa", "pkg"]],
  ["image", [
    "avif", "bmp", "cr2", "dng", "gif", "heic", "heif", "ico", "jpeg", "jpg", "jxl", "png", "psd", "raw", "svg", "tif", "tiff", "webp",
  ]],
  ["audio", ["aac", "aiff", "alac", "amr", "ape", "flac", "m4a", "mid", "midi", "mp3", "oga", "ogg", "opus", "wav", "wma"]],
  ["video", ["3gp", "3g2", "avi", "flv", "m2ts", "m4v", "mkv", "mov", "mp4", "mpeg", "mpg", "mts", "ogv", "webm", "wmv"]],
  ["code", [
    "asm", "bash", "bat", "c", "cc", "cfg", "clj", "cls", "cmake", "coffee", "conf", "cpp", "cs", "css", "cxx",
    "dart", "diff", "dockerfile", "env", "fish", "fs", "go", "gradle", "groovy", "h", "hcl", "hpp", "html", "htm",
    "ini", "java", "jl", "js", "json", "jsx", "kt", "kts", "less", "log", "lua", "m", "make", "md", "mdx", "mjs", "mm",
    "php", "pl", "pm", "properties", "ps1", "py", "r", "rb", "rs", "sass", "scala", "scss", "sh", "sql", "svelte",
    "swift", "toml", "ts", "tsx", "vue", "xml", "yaml", "yml", "zsh",
  ]],
] as const;

export type FileIconCategory = (typeof extensionGroups)[number][0] | "generic";

const categoryByExtension = new Map<string, FileIconCategory>();
for (const [category, extensions] of extensionGroups) {
  for (const extension of extensions) categoryByExtension.set(extension, category);
}

const codeFileNames = new Set([
  ".babelrc", ".editorconfig", ".env", ".eslintrc", ".gitignore", ".npmrc", ".prettierrc", "dockerfile", "makefile", "rakefile",
]);

export function clipFileName(path: string) {
  const normalizedPath = removeTrailingSeparators(path);
  if (!normalizedPath) return "";

  const separatorIndex = isWindowsPath(normalizedPath)
    ? Math.max(normalizedPath.lastIndexOf("\\"), normalizedPath.lastIndexOf("/"))
    : normalizedPath.lastIndexOf("/");

  return normalizedPath.slice(separatorIndex + 1);
}

export function clipFileExtension(path: string) {
  return extensionFromFileName(clipFileName(path));
}

export function fileIconCategory(path: string): FileIconCategory {
  const fileName = clipFileName(path);
  return categoryByExtension.get(extensionFromFileName(fileName))
    ?? (codeFileNames.has(fileName.toLowerCase()) ? "code" : "generic");
}

export function fileIconSrc(path: string) {
  return `/file-icons/${fileIconCategory(path)}.webp`;
}

function extensionFromFileName(fileName: string) {
  const extensionIndex = fileName.lastIndexOf(".");
  if (extensionIndex <= 0 || extensionIndex === fileName.length - 1) return "";

  return fileName.slice(extensionIndex + 1).toLowerCase();
}

function isWindowsPath(path: string) {
  return /^[a-zA-Z]:[\\/]|^\\\\/.test(path);
}

function removeTrailingSeparators(path: string) {
  return isWindowsPath(path) ? path.replace(/[\\/]+$/, "") : path.replace(/\/+$/, "");
}
