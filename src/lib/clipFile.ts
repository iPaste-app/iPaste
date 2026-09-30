const documentExtensions = new Set([
  "doc", "docm", "docx", "dot", "dotm", "dotx", "epub", "odt", "pages", "pdf", "rtf", "tex", "txt",
]);
const spreadsheetExtensions = new Set([
  "csv", "numbers", "ods", "tsv", "xls", "xlsb", "xlsm", "xlsx", "xlt", "xltm", "xltx",
]);
const presentationExtensions = new Set([
  "key", "odp", "pot", "potm", "potx", "pps", "ppsm", "ppsx", "ppt", "pptm", "pptx",
]);
const archiveExtensions = new Set([
  "7z", "ar", "bz", "bz2", "cab", "gz", "iso", "lz", "lz4", "rar", "tar", "tbz", "tgz", "txz", "xz", "z", "zip", "zst",
]);
const imageExtensions = new Set([
  "avif", "bmp", "cr2", "dng", "gif", "heic", "heif", "ico", "jpeg", "jpg", "jxl", "png", "psd", "raw", "svg", "tif", "tiff", "webp",
]);
const audioExtensions = new Set([
  "aac", "aiff", "alac", "amr", "ape", "flac", "m4a", "mid", "midi", "mp3", "oga", "ogg", "opus", "wav", "wma",
]);
const videoExtensions = new Set([
  "3gp", "3g2", "avi", "flv", "m2ts", "m4v", "mkv", "mov", "mp4", "mpeg", "mpg", "mts", "ogv", "webm", "wmv",
]);
const codeExtensions = new Set([
  "asm", "bash", "bat", "c", "cc", "cfg", "clj", "cls", "cmake", "coffee", "conf", "cpp", "cs", "css", "cxx", "dart", "diff", "dockerfile", "env", "fish", "fs", "go", "gradle", "groovy", "h", "hcl", "hpp", "html", "htm", "ini", "java", "jl", "js", "json", "jsx", "kt", "kts", "less", "log", "lua", "m", "make", "md", "mdx", "mjs", "mm", "php", "pl", "pm", "properties", "ps1", "py", "r", "rb", "rs", "sass", "scala", "scss", "sh", "sql", "svelte", "swift", "toml", "ts", "tsx", "vue", "xml", "yaml", "yml", "zsh",
]);
const codeFileNames = new Set([
  ".babelrc", ".editorconfig", ".env", ".eslintrc", ".gitignore", ".npmrc", ".prettierrc", "dockerfile", "makefile", "rakefile",
]);

export type FileIconCategory =
  | "document"
  | "pdf"
  | "spreadsheet"
  | "presentation"
  | "archive"
  | "image"
  | "audio"
  | "video"
  | "code"
  | "generic";

export function clipFileName(path: string) {
  const normalizedPath = removeTrailingSeparators(path);
  if (!normalizedPath) return "";

  const separatorIndex = isWindowsPath(normalizedPath)
    ? Math.max(normalizedPath.lastIndexOf("\\"), normalizedPath.lastIndexOf("/"))
    : normalizedPath.lastIndexOf("/");

  return normalizedPath.slice(separatorIndex + 1);
}

export function clipFileExtension(path: string) {
  const fileName = clipFileName(path);
  const extensionIndex = fileName.lastIndexOf(".");
  if (extensionIndex <= 0 || extensionIndex === fileName.length - 1) return "";

  return fileName.slice(extensionIndex + 1).toLowerCase();
}

export function fileIconCategory(path: string): FileIconCategory {
  const extension = clipFileExtension(path);
  const fileName = clipFileName(path).toLowerCase();

  if (extension === "pdf") return "pdf";
  if (documentExtensions.has(extension)) return "document";
  if (spreadsheetExtensions.has(extension)) return "spreadsheet";
  if (presentationExtensions.has(extension)) return "presentation";
  if (archiveExtensions.has(extension)) return "archive";
  if (imageExtensions.has(extension)) return "image";
  if (audioExtensions.has(extension)) return "audio";
  if (videoExtensions.has(extension)) return "video";
  if (codeExtensions.has(extension) || codeFileNames.has(fileName)) return "code";
  return "generic";
}

export function fileIconSrc(path: string) {
  return `/file-icons/${fileIconCategory(path)}.webp`;
}

function isWindowsPath(path: string) {
  return /^[a-zA-Z]:[\\/]|^\\\\/.test(path);
}

function removeTrailingSeparators(path: string) {
  return isWindowsPath(path) ? path.replace(/[\\/]+$/, "") : path.replace(/\/+$/, "");
}
