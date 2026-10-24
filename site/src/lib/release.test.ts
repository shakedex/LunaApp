import { describe, expect, it } from "vitest";
import { formatMb, parseRelease } from "./release";

describe("parseRelease", () => {
  it("picks the two installers out of a release", () => {
    const r = parseRelease({
      tag_name: "v2026.10.0",
      assets: [
        { name: "Luna-2026.10.0-windows-x64-setup.exe", browser_download_url: "https://x/win", size: 160_000_000 },
        { name: "Luna-2026.10.0-macos.dmg", browser_download_url: "https://x/mac", size: 170_000_000 },
        { name: "Luna-2026.10.0-macos-update.zip", browser_download_url: "https://x/zip", size: 1 },
        { name: "appcast-windows.xml", browser_download_url: "https://x/xml", size: 1 },
      ],
    });
    expect(r?.version).toBe("2026.10.0");
    expect(r?.windows?.name).toBe("Luna-2026.10.0-windows-x64-setup.exe");
    expect(r?.macos?.url).toBe("https://x/mac");
  });
  it("returns null without a tag", () => {
    expect(parseRelease({})).toBeNull();
  });
});

describe("formatMb", () => {
  it("quotes decimal megabytes", () => {
    expect(formatMb(163_400_000)).toBe("163 MB");
    expect(formatMb(9_400_000)).toBe("9.4 MB");
  });
});
