import { describe, expect, it } from "vitest";
import { detectPlatform, downloadLabels } from "./platform";

describe("detectPlatform", () => {
  it("reads a Mac user agent as macOS", () => {
    expect(detectPlatform("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/605.1.15")).toBe("macos");
  });
  it("reads a Windows user agent as Windows", () => {
    expect(detectPlatform("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")).toBe("windows");
  });
  it("falls back to Windows for anything else", () => {
    expect(detectPlatform("Mozilla/5.0 (X11; Linux x86_64)")).toBe("windows");
    expect(detectPlatform("")).toBe("windows");
  });
});

describe("downloadLabels", () => {
  it("names the visitor's system first and the other second", () => {
    expect(downloadLabels("macos")).toEqual({ primary: "Download for macOS", secondary: "Windows" });
    expect(downloadLabels("windows")).toEqual({ primary: "Download for Windows", secondary: "macOS" });
  });
});
