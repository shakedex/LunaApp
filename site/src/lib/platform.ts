export type Platform = "windows" | "macos" | "other";

/** The visitor's desktop platform from the user agent; phones count as other. */
export function detectPlatform(userAgent: string): Platform {
  const ua = userAgent.toLowerCase();
  if (/iphone|ipad|ipod|android/.test(ua)) return "other";
  if (ua.includes("windows")) return "windows";
  if (ua.includes("mac os") || ua.includes("macintosh")) return "macos";
  return "other";
}

export interface DownloadLabels {
  primary: string;
  secondary: string;
}

/** The main button names the visitor's platform; the other one is a link. */
export function downloadLabels(platform: Platform): DownloadLabels {
  if (platform === "macos") return { primary: "Download for macOS", secondary: "Windows" };
  return { primary: "Download for Windows", secondary: "macOS" };
}
