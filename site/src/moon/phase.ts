// The app's lighting model (crates/luna-app/src/moon.rs): the lit share of the
// disc sets the angle between the sun and the viewer.

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

/** View-space sun direction for a moon `lit` from 0 (new) to 1 (full): x right, y up, z toward the viewer. */
export function phaseToSunDirection(lit: number): [number, number, number] {
  const phi = Math.acos(1 - 2 * clamp01(lit));
  return [Math.sin(phi), 0, -Math.cos(phi)];
}

/** Dial ticks lit at `progress` (0 to 1), clockwise from the top. */
export function ticksLit(progress: number, ticks: number): number {
  return Math.floor(clamp01(progress) * ticks + 1e-9);
}
