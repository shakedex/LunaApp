import { describe, expect, it } from "vitest";
import { detectPlatform, downloadLabels } from "./platform";

describe("detectPlatform", () => {
  it("finds Windows", () => {
    expect(detectPlatform("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/130")).toBe("windows");
  });
  it("finds macOS", () => {
    expect(detectPlatform("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Safari/605")).toBe("macos");
  });
  it("treats phones as other, even when they say Mac OS", () => {
    expect(detectPlatform("Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)")).toBe("other");
    expect(detectPlatform("Mozilla/5.0 (Linux; Android 14) Chrome/130")).toBe("other");
  });
  it("treats Linux as other", () => {
    expect(detectPlatform("Mozilla/5.0 (X11; Linux x86_64) Firefox/131")).toBe("other");
  });
});

describe("downloadLabels", () => {
  it("names the visitor's platform first", () => {
    expect(downloadLabels("macos")).toEqual({ primary: "Download for macOS", secondary: "Windows" });
    expect(downloadLabels("windows")).toEqual({ primary: "Download for Windows", secondary: "macOS" });
  });
  it("defaults to Windows", () => {
    expect(downloadLabels("other").primary).toBe("Download for Windows");
  });
});
