// The night sky behind the page, drawn live: the app's star chart as a
// turning celestial sphere. A graticule of meridians and parallels on a
// sphere much larger than the screen, seen from just inside it, turns about
// a tilted axis once every twenty minutes; the stars sit on the same sphere
// and breathe a little. Reduced motion gets one still frame.

const LINE = "121, 202, 234"; // the app's accent, very faint
const STAR = "228, 232, 239"; // moonlight
const TURN_SECONDS = 1200;
const TILT = (-14 * Math.PI) / 180;
const STARS = 420;

interface Star {
  lat: number;
  lon: number;
  size: number;
  bright: number;
  phase: number;
  rate: number;
}

function hash(i: number, salt: number): number {
  let h = (Math.imul(i + 1, 374761393) + Math.imul(salt + 7, 668265263)) >>> 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177) >>> 0;
  return (h & 0xffff) / 65535;
}

export function runSky(canvas: HTMLCanvasElement) {
  const ctx = canvas.getContext("2d", { alpha: true });
  if (!ctx) return;
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;

  const stars: Star[] = Array.from({ length: STARS }, (_, i) => ({
    // Even over the sphere: uniform in sin(lat).
    lat: Math.asin(2 * hash(i, 1) - 1),
    lon: hash(i, 2) * Math.PI * 2,
    size: 0.6 + hash(i, 3) ** 3 * 1.6,
    bright: 0.25 + hash(i, 4) ** 2 * 0.6,
    phase: hash(i, 5) * Math.PI * 2,
    rate: 0.3 + hash(i, 6) * 0.9,
  }));

  let w = 0;
  let h = 0;
  let dpr = 1;
  const resize = () => {
    dpr = Math.min(1.5, devicePixelRatio || 1);
    w = innerWidth;
    h = innerHeight;
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    canvas.style.width = `${w}px`;
    canvas.style.height = `${h}px`;
  };
  resize();
  addEventListener("resize", resize);

  // The sphere: centred below and right of the screen, so the lines sweep in
  // gentle arcs like the app's, with the viewer looking at its near face.
  const project = (lat: number, lon: number, turn: number, out: number[]) => {
    const r = Math.max(w, h) * 1.35;
    const cx = w * 0.62;
    const cy = h * 0.95;
    const cl = Math.cos(lat);
    let x = r * cl * Math.sin(lon + turn);
    let y = r * Math.sin(lat);
    const z = r * cl * Math.cos(lon + turn);
    // Tilt the axis.
    const ty = y * Math.cos(TILT) - x * Math.sin(TILT);
    const tx = y * Math.sin(TILT) + x * Math.cos(TILT);
    x = tx;
    y = ty;
    out[0] = cx + x;
    out[1] = cy - y;
    out[2] = z;
  };

  const p = [0, 0, 0];
  const draw = (t: number) => {
    const turn = ((t / 1000) / TURN_SECONDS) * Math.PI * 2;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);

    // Graticule: parallels every 10°, meridians every 10°, only the near face.
    ctx.lineWidth = 1;
    const line = (points: (s: number) => void, steps: number) => {
      ctx.beginPath();
      let pen = false;
      for (let i = 0; i <= steps; i++) {
        points(i / steps);
        if (p[2] <= 0) {
          pen = false;
          continue;
        }
        if (pen) ctx.lineTo(p[0], p[1]);
        else ctx.moveTo(p[0], p[1]);
        pen = true;
      }
      ctx.stroke();
    };
    for (let k = -8; k <= 8; k++) {
      const lat = (k * 10 * Math.PI) / 180;
      ctx.strokeStyle = `rgba(${LINE}, ${k % 3 === 0 ? 0.09 : 0.05})`;
      line((s) => project(lat, s * Math.PI * 2, turn, p), 240);
    }
    for (let k = 0; k < 36; k++) {
      const lon = (k * 10 * Math.PI) / 180;
      ctx.strokeStyle = `rgba(${LINE}, ${k % 3 === 0 ? 0.09 : 0.05})`;
      line((s) => project(-Math.PI / 2 + s * Math.PI, lon, turn, p), 120);
    }

    // Stars, with a slow breath each.
    for (const s of stars) {
      project(s.lat, s.lon, turn * 0.8, p);
      if (p[2] <= 0) continue;
      const tw = 0.75 + 0.25 * Math.sin((t / 1000) * s.rate + s.phase);
      ctx.fillStyle = `rgba(${STAR}, ${(s.bright * tw).toFixed(3)})`;
      ctx.beginPath();
      ctx.arc(p[0], p[1], s.size, 0, Math.PI * 2);
      ctx.fill();
    }
  };

  if (reduce) {
    draw(0);
    return;
  }
  let last = 0;
  const frame = (t: number) => {
    // 30 fps is plenty for a sky.
    if (t - last > 33) {
      last = t;
      if (document.visibilityState === "visible") draw(t);
    }
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
}
