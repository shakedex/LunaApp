// The latest GitHub release, for the version and installer names on the
// Download section. The page is complete without it; this only fills in.

export const REPO = "shakedex/LunaApp";
export const RELEASES = `https://github.com/${REPO}/releases`;
export const LATEST = `${RELEASES}/latest`;

export interface Release {
  version: string;
  windows?: { name: string; url: string; bytes: number };
  macos?: { name: string; url: string; bytes: number };
}

interface Asset {
  name: string;
  browser_download_url: string;
  size: number;
}

/** Reads a GitHub releases API response into the two installers. */
export function parseRelease(json: { tag_name?: string; assets?: Asset[] }): Release | null {
  const tag = json.tag_name;
  if (!tag) return null;
  const version = tag.replace(/^v/, "");
  const pick = (test: RegExp) => {
    const a = json.assets?.find((x) => test.test(x.name));
    return a ? { name: a.name, url: a.browser_download_url, bytes: a.size } : undefined;
  };
  return {
    version,
    windows: pick(/windows.*\.exe$/i),
    macos: pick(/\.dmg$/i),
  };
}

export async function fetchLatestRelease(): Promise<Release | null> {
  try {
    const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!res.ok) return null;
    return parseRelease(await res.json());
  } catch {
    return null;
  }
}

/** Decimal megabytes, one decimal: the unit installers are quoted in. */
export function formatMb(bytes: number): string {
  return `${(bytes / 1e6).toFixed(bytes >= 1e8 ? 0 : 1)} MB`;
}
