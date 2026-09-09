import { openUrl } from "@tauri-apps/plugin-opener";

export const IPASTE_GITHUB_REPOSITORY_URL = "https://github.com/iPaste-app/iPaste";
export const IPASTE_GITHUB_RELEASES_URL = `${IPASTE_GITHUB_REPOSITORY_URL}/releases`;

export function getGitHubReleaseUrl(version: string) {
  const normalizedVersion = version.trim().replace(/^ipaste-v/i, "").replace(/^v/i, "");
  if (!normalizedVersion) return IPASTE_GITHUB_RELEASES_URL;

  const releaseTag = `iPaste-v${normalizedVersion}`;
  return `${IPASTE_GITHUB_RELEASES_URL}/tag/${encodeURIComponent(releaseTag)}`;
}

export async function openGitHubRepository() {
  await openExternalUrl(IPASTE_GITHUB_REPOSITORY_URL);
}

export async function openGitHubRelease(version: string) {
  await openExternalUrl(getGitHubReleaseUrl(version));
}

export async function openExternalUrl(url: string) {
  if ("__TAURI_INTERNALS__" in window) {
    await openUrl(url);
    return;
  }

  window.open(url, "_blank", "noopener,noreferrer");
}
