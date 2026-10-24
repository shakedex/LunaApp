import { describe, expect, it } from "vitest";
import { phaseToSunDirection, ticksLit } from "./phase";

describe("phaseToSunDirection", () => {
  it("lights a full moon from the viewer", () => {
    const [x, y, z] = phaseToSunDirection(1);
    expect(x).toBeCloseTo(0, 5);
    expect(y).toBe(0);
    expect(z).toBeCloseTo(1, 5);
  });
  it("lights a half moon from the right", () => {
    const [x, , z] = phaseToSunDirection(0.5);
    expect(x).toBeCloseTo(1, 5);
    expect(z).toBeCloseTo(0, 5);
  });
  it("puts the sun behind a thin crescent", () => {
    expect(phaseToSunDirection(0.04)[2]).toBeLessThan(-0.9);
  });
  it("clamps out-of-range phases", () => {
    expect(phaseToSunDirection(2)).toEqual(phaseToSunDirection(1));
    expect(phaseToSunDirection(-1)).toEqual(phaseToSunDirection(0));
  });
});

describe("ticksLit", () => {
  it("counts whole ticks reached", () => {
    expect(ticksLit(0, 72)).toBe(0);
    expect(ticksLit(0.68, 72)).toBe(48);
    expect(ticksLit(1, 72)).toBe(72);
    expect(ticksLit(1.4, 72)).toBe(72);
  });
});
