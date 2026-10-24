import { describe, expect, it } from "vitest";
import { lerpRect, windowMoonRect } from "./dock";

const layout = { moonCx: 880, moonCy: 520, moonBox: 600, w: 1760, h: 1280 };

describe("windowMoonRect", () => {
  it("places the Moon square where the app draws it, scaled to the shown window", () => {
    expect(windowMoonRect({ left: 100, top: 50, width: 880 }, layout)).toEqual({ x: 390, y: 160, size: 300 });
  });
  it("follows the window at full size", () => {
    expect(windowMoonRect({ left: 0, top: 0, width: 1760 }, layout)).toEqual({ x: 580, y: 220, size: 600 });
  });
});

describe("lerpRect", () => {
  it("blends two rects", () => {
    const a = { x: 0, y: 0, size: 100 };
    const b = { x: 100, y: 200, size: 300 };
    expect(lerpRect(a, b, 0)).toEqual(a);
    expect(lerpRect(a, b, 1)).toEqual(b);
    expect(lerpRect(a, b, 0.5)).toEqual({ x: 50, y: 100, size: 200 });
  });
});
