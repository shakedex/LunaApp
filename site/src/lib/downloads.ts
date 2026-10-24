// Every Download button on the page points at an installer file, never at
// a GitHub page. The static links name the release the page was built with;
// once GitHub answers, they move to the latest release.
import { KNOWN_VERSION, RELEASES_URL } from "../copy";
import { detectPlatform, downloadLabels, type Platform } from "./platform";
import { fetchLatestRelease } from "./release";

export type Installer = "windows" | "macos";

export const installerName = (platform: Installer, version: string) =>
  platform === "windows" ? `Luna-${version}-windows-x64-setup.exe` : `Luna-${version}-macos.dmg`;

/** The direct file URL of a release's installer. */
export const installerUrl = (platform: Installer, version = KNOWN_VERSION) =>
  `${RELEASES_URL}/download/v${version}/${installerName(platform, version)}`;

const urls: Record<Installer, string> = {
  windows: installerUrl("windows"),
  macos: installerUrl("macos"),
};

/** Buttons carry data-dl: "windows", "macos", "primary" (the visitor's) or "other". */
export function applyDownloads(platform: Platform) {
  const primary: Installer = platform === "macos" ? "macos" : "windows";
  const other: Installer = primary === "macos" ? "windows" : "macos";
  const labels = downloadLabels(platform);
  document.documentElement.dataset.os = primary;
  document.querySelectorAll<HTMLAnchorElement>("[data-dl]").forEach((a) => {
    const which = a.dataset.dl as Installer | "primary" | "other";
    const target = which === "primary" ? primary : which === "other" ? other : which;
    a.href = urls[target];
    a.setAttribute("download", "");
  });
  document.querySelectorAll("[data-download-label]").forEach((el) => (el.textContent = labels.primary));
  document.querySelectorAll("[data-download-other]").forEach((el) => (el.textContent = labels.secondary));
}

export function setupDownloads() {
  const platform = detectPlatform(navigator.userAgent);
  applyDownloads(platform);
  fetchLatestRelease().then((release) => {
    if (!release) return;
    if (release.windows) urls.windows = release.windows.url;
    if (release.macos) urls.macos = release.macos.url;
    applyDownloads(platform);
    const version = document.querySelector("[data-release-version]");
    if (version) version.textContent = `Version ${release.version}`;
  });
}
