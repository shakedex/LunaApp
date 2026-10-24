export type Platform = "windows" | "macos";

const LABEL: Record<Platform, string> = { windows: "Windows", macos: "macOS" };

/** Luna ships for Windows and macOS; any other system gets the Windows build named first. */
export function detectPlatform(userAgent: string): Platform {
  return /Macintosh|Mac OS X/.test(userAgent) ? "macos" : "windows";
}

export function downloadLabels(platform: Platform): { primary: string; secondary: string } {
  const other: Platform = platform === "macos" ? "windows" : "macos";
  return { primary: `Download for ${LABEL[platform]}`, secondary: LABEL[other] };
}
