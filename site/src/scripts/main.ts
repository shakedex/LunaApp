// Turns the static page into the moving one: only on wide screens with motion
// allowed and WebGL working. Everything else keeps the still page as built.

import { counterText } from "../moon/counter";
import type { Moon } from "../moon/scene";

const MOTION = "(min-width: 1024px) and (prefers-reduced-motion: no-preference)";
const TOTAL_CLIPS = 25;

/** What the Moon shows; the hero intro and the scroll timeline both write here. */
export interface MoonState {
  lit: number;
  progress: number;
  /** 0 on the hero slot, 1 in the app window. */
  dock: number;
  /** Canvas opacity: the report page hides the Moon at the end of the sequence. */
  shown: number;
}

/** Where the Moon is this frame, and how visible. */
export interface Placement {
  rect: { x: number; y: number; size: number };
  opacity: number;
}

async function start() {
  if (!matchMedia(MOTION).matches) return;
  const canvas = document.getElementById("moon-canvas") as HTMLCanvasElement;
  const still = document.getElementById("moon-still") as HTMLImageElement;
  const count = document.getElementById("clip-count")!;
  const sub = document.getElementById("clip-sub")!;

  const [{ createMoon }, { gsap }] = await Promise.all([import("../moon/scene"), import("gsap")]);
  let lost = false;
  const fallBack = () => {
    lost = true;
    document.documentElement.classList.remove("motion");
    canvas.style.opacity = "0";
    still.style.opacity = "1";
  };
  const moon: Moon | null = await createMoon(canvas, fallBack);
  if (!moon) return;

  const state: MoonState = { lit: 0.04, progress: 0, dock: 0, shown: 1 };
  const slot = document.getElementById("hero-moon-slot")!;
  let last = "";
  const sync = (rect: { x: number; y: number; size: number }) => {
    const key = `${rect.x.toFixed(1)} ${rect.y.toFixed(1)} ${rect.size.toFixed(1)} ${state.lit.toFixed(4)} ${state.progress.toFixed(4)}`;
    if (key === last) return;
    last = key;
    moon.setRect(rect.x, rect.y, rect.size);
    moon.setPhase(state.lit);
    moon.setProgress(state.progress);
  };
  const heroRect = () => {
    const r = slot.getBoundingClientRect();
    return { x: r.left, y: r.top, size: r.width };
  };

  let place = (): Placement => ({ rect: heroRect(), opacity: 1 });
  let shownStarted = false;
  gsap.ticker.add(() => {
    if (lost) return;
    const p = place();
    sync(p.rect);
    if (shownStarted) canvas.style.opacity = String(p.opacity);
  });

  // Hero: one run from a thin crescent to the report, with the clip counter.
  const intro = gsap.to(state, {
    lit: 1,
    progress: 1,
    duration: 4.2,
    delay: 0.4,
    ease: "power2.out",
    onUpdate: () => {
      const t = counterText(state.progress, TOTAL_CLIPS);
      count.textContent = t.count;
      sub.textContent = t.sub;
    },
  });
  sync(heroRect());
  requestAnimationFrame(() => {
    canvas.style.opacity = "1";
    still.style.opacity = "0";
    // After the fade-in, opacity follows the page; drop the CSS transition so it tracks scroll.
    setTimeout(() => {
      canvas.style.transition = "none";
      shownStarted = true;
    }, 700);
  });

  const { setupScroll } = await import("./scroll");
  place = setupScroll({ gsap, state, intro, heroRect });
}

start();
