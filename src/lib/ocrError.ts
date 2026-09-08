export type OcrModelIssue = {
  kind: "download" | "repair";
  missingFiles: string[];
};

export function getOcrModelIssue(error: unknown): OcrModelIssue | null {
  if (!error || typeof error !== "object" || !("code" in error)) return null;
  if (error.code !== "models_missing" && error.code !== "models_incomplete") return null;
  const missingFiles = "missingFiles" in error && Array.isArray(error.missingFiles)
    ? error.missingFiles.filter((file): file is string => typeof file === "string")
    : [];
  return { kind: error.code === "models_missing" ? "download" : "repair", missingFiles };
}

export function getOcrErrorMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error && typeof error.message === "string") {
    return error.message;
  }
  return String(error);
}
