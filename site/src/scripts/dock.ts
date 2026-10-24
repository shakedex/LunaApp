/** A Moon square in viewport CSS pixels. */
export interface Rect {
  x: number;
  y: number;
  size: number;
}

/** Where the capture tool found the Moon in the app window (public/img/layout.json), in capture pixels. */
export interface Layout {
  moonCx: number;
  moonCy: number;
  moonBox: number;
  w: number;
  h: number;
}

/** The Moon's square inside the app window shown at `win` (its bounding box). */
export function windowMoonRect(win: { left: number; top: number; width: number }, layout: Layout): Rect {
  const k = win.width / layout.w;
  const size = layout.moonBox * k;
  return { x: win.left + layout.moonCx * k - size / 2, y: win.top + layout.moonCy * k - size / 2, size };
}

export function lerpRect(a: Rect, b: Rect, t: number): Rect {
  return { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t, size: a.size + (b.size - a.size) * t };
}
