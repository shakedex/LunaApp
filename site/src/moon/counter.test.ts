import { describe, expect, it } from "vitest";
import { counterText } from "./counter";

describe("counterText", () => {
  it("starts at the first reel", () => {
    expect(counterText(0, 25)).toEqual({ count: "0 of 25 clips", sub: "Reading A003" });
  });
  it("names the reel being read", () => {
    expect(counterText(0.68, 25)).toEqual({ count: "17 of 25 clips", sub: "Reading A004" });
    expect(counterText(0.8, 25).sub).toBe("Reading B002");
  });
  it("ends with the report", () => {
    expect(counterText(1, 25)).toEqual({ count: "Report ready", sub: "PDF, HTML and CSV" });
  });
});
