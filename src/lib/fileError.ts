import { t, type I18nKey } from "../i18n";

const fileErrorKeys: Record<string, I18nKey> = {
  FILE_NOT_FOUND: "file.error.notFound",
  FILE_ACCESS_DENIED: "file.error.accessDenied",
  FILE_NOT_REGULAR: "file.error.notRegular",
  FILE_UNAVAILABLE: "file.error.unavailable",
  FILE_INVALID_PATH: "file.error.invalidPath",
  FILE_READ_ONLY: "file.error.readOnly",
  FILE_CLIPBOARD_WRITE: "file.error.clipboardWrite",
  FILE_NATIVE_ONLY: "file.error.nativeOnly",
};

export function fileErrorMessage(error: unknown) {
  const key = fileErrorKeys[String(error)];
  return key ? t(key) : null;
}

export function localizeFileError(error: unknown): never {
  throw fileErrorMessage(error) ?? error;
}
