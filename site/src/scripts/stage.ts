// The pinned sequence at the top of the page. Scroll drives one timeline:
// the intro gives way to the three steps, the window goes from waiting to
// reading to done with the live Moon filling in step, and the report's first
// page rises out of it. The Moon turns slowly on its own the whole time.
import { gsap } from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import { copy } from "../copy";
import { detectPlatform, downloadLabels } from "../lib/platform";
import type { Moon } from "../moon/scene";

interface Rows {
  moon: { cx: number; cy: number; box: number };
  titleTop: number;
  subTop: number;
}
interface Layout {
  width: number;
  height: number;
  "hero-idle": Rows;
  "hero-running": Rows;
  "hero-done": Rows;
}

const TOTAL = 25;
const TRAVEL = 3400; // px of scroll for the whole sequence
const SPIN_PERIOD = 90; // seconds per turn, the app's idle speed

function labelDownloads() {
  const labels = downloadLabels(detectPlatform(navigator.userAgent));
  document.querySelectorAll("[data-download-label]").forEach((el) => (el.textContent = labels.primary));
  document.querySelectorAll("[data-download-other]").forEach((el) => (el.textContent = labels.secondary));
}

export function runStage() {
  labelDownloads();
  const stage = document.getElementById("how");
  const pin = document.getElementById("stage-pin");
  const intro = document.getElementById("stage-intro");
  const steps = document.getElementById("stage-steps");
  const win = document.getElementById("stage-window");
  const canvas = document.getElementById("stage-moon") as HTMLCanvasElement | null;
  const line1 = document.getElementById("stage-line-1");
  const line2 = document.getElementById("stage-line-2");
  const page = document.getElementById("stage-page");
  if (!stage || !pin || !intro || !steps || !win || !canvas || !line1 || !line2 || !page) return;

  const wide = matchMedia("(min-width: 1000px)").matches;
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (!wide || reduce) return;

  const layout = JSON.parse(stage.dataset.layout ?? "{}") as Layout;
  const layers = {
    idle: win.querySelector<HTMLImageElement>('[data-phase="idle"]')!,
    running: win.querySelector<HTMLImageElement>('[data-phase="running"]')!,
    done: win.querySelector<HTMLImageElement>('[data-phase="done"]')!,
  };

  import("../moon/scene")
    .then(({ createMoon }) => createMoon(canvas, "/img/moon-albedo.webp"))
    .then((moon) => {
      if (!moon) return; // the plain column stays
      // The blank screenshots: the Moon and the lines are drawn live.
      for (const img of Object.values(layers)) img.src = img.dataset.live!;
      canvas.hidden = false;
      line1.hidden = false;
      line2.hidden = false;
      document.documentElement.classList.add("motion");
      build({ layout, layers, intro, steps, win, canvas, line1, line2, page, pin, stage, moon });
    })
    .catch((e) => console.warn("Luna: the live Moon failed, the plain page stays", e));
}

interface Parts {
  layout: Layout;
  layers: Record<"idle" | "running" | "done", HTMLImageElement>;
  intro: HTMLElement;
  steps: HTMLElement;
  win: HTMLElement;
  canvas: HTMLCanvasElement;
  line1: HTMLElement;
  line2: HTMLElement;
  page: HTMLElement;
  pin: HTMLElement;
  stage: HTMLElement;
  moon: Moon;
}

function build(p: Parts) {
  gsap.registerPlugin(ScrollTrigger);
  const { layout, layers, moon } = p;
  const w = copy.stage.window;
  const pct = (v: number, of: number) => (v / of) * 100;

  // Where the Moon and the lines sit in each phase, as shares of the window.
  const place = (rows: Rows) => ({
    moonTop: pct(rows.moon.cy - rows.moon.box / 2, layout.height),
    moonLeft: pct(rows.moon.cx - rows.moon.box / 2, layout.width),
    moonSize: pct(rows.moon.box, layout.width),
    titleTop: pct(rows.titleTop, layout.height),
    subTop: pct(rows.subTop, layout.height),
  });
  const idle = place(layout["hero-idle"]);
  const running = place(layout["hero-running"]);
  const done = place(layout["hero-done"]);
  const pos = { ...idle };
  const applyPos = () => {
    p.canvas.style.left = `${pos.moonLeft}%`;
    p.canvas.style.width = `${pos.moonSize}%`;
    p.canvas.style.top = `${pos.moonTop}%`;
    // The glyphs sit a little below the top of their line box.
    p.line1.style.top = `calc(${pos.titleTop}% - 0.3cqw)`;
    p.line2.style.top = `calc(${pos.subTop}% - 0.2cqw)`;
  };
  applyPos();

  // What the window says, and how lit the Moon is.
  const state = { phase: "idle" as "idle" | "running" | "done", progress: 0 };
  const applyText = () => {
    if (state.phase === "idle") {
      p.line1.textContent = w.idle[0];
      p.line2.textContent = w.idle[1];
    } else if (state.phase === "done") {
      p.line1.textContent = w.ready;
      p.line2.textContent = w.summary;
    } else {
      const clips = Math.round(state.progress * TOTAL);
      p.line1.textContent = state.progress < 0.02 ? w.looking : w.clips(clips, TOTAL);
      p.line2.textContent = w.source;
    }
  };
  applyText();
  const setPhase = (phase: typeof state.phase) => {
    if (state.phase === phase) return;
    state.phase = phase;
    applyText();
    const step = phase === "idle" ? 0 : phase === "running" ? 1 : 2;
    p.steps.querySelectorAll("li").forEach((li, i) => li.classList.toggle("is-on", i === step));
  };

  // The Moon, drawn every frame while the stage is on screen so it turns.
  let visible = true;
  let frame = 0;
  const start = performance.now();
  const draw = (now: number) => {
    frame = 0;
    const spin = ((now - start) / 1000 / SPIN_PERIOD) * Math.PI * 2;
    const lit = state.phase === "done" ? 1 : state.phase === "idle" ? 0.08 : 0.08 + 0.92 * state.progress;
    const progress = state.phase === "done" ? 1 : state.phase === "idle" ? 0 : state.progress;
    moon.render({ lit, spin, progress });
    if (visible) frame = requestAnimationFrame(draw);
  };
  frame = requestAnimationFrame(draw);
  new IntersectionObserver(([e]) => {
    visible = e.isIntersecting && document.visibilityState === "visible";
    if (visible && !frame) frame = requestAnimationFrame(draw);
  }).observe(p.win);
  document.addEventListener("visibilitychange", () => {
    visible = document.visibilityState === "visible";
    if (visible && !frame) frame = requestAnimationFrame(draw);
  });
  addEventListener("resize", () => moon.resize());

  // Centred on the pinned frame, and parked below the viewport to start.
  gsap.set(p.page, { xPercent: -50, yPercent: -50, y: () => innerHeight * 1.2 });

  const tl = gsap.timeline({
    defaults: { ease: "none" },
    scrollTrigger: {
      trigger: p.stage,
      start: "top 64px",
      end: `+=${TRAVEL}`,
      pin: p.pin,
      scrub: 0.6,
      anticipatePin: 1,
      invalidateOnRefresh: true,
      onUpdate: (self) => {
        const t = self.progress;
        setPhase(t < 0.16 ? "idle" : t < 0.76 ? "running" : "done");
        applyText();
      },
    },
  });

  tl
    // The intro gives way to the steps.
    .to(p.intro, { autoAlpha: 0, y: -24, duration: 0.1, ease: "power2.in" }, 0)
    .to(p.steps, { autoAlpha: 1, duration: 0.08 }, 0.06)
    // A folder is dropped: the window starts reading. The app switches
    // screens in one frame, so the layers cut rather than fade.
    .set(layers.idle, { opacity: 0 }, 0.16)
    .set(layers.running, { opacity: 1 }, 0.16)
    .set(pos, { ...running, onUpdate: applyPos }, 0.16)
    // The clips are read: the Moon fills.
    .to(state, { progress: 1, duration: 0.54 }, 0.2)
    // Done.
    .set(layers.running, { opacity: 0 }, 0.76)
    .set(layers.done, { opacity: 1 }, 0.76)
    .set(pos, { ...done, onUpdate: applyPos }, 0.76)
    // The first page rises out of the window.
    .to(p.page, { y: 0, duration: 0.16, ease: "power2.out" }, 0.84)
    .to(p.win, { opacity: 0.12, scale: 0.94, duration: 0.14 }, 0.84)
    .to(p.steps, { autoAlpha: 0, duration: 0.08 }, 0.88);

  // Fonts and images change the layout as they arrive.
  document.fonts?.ready.then(() => ScrollTrigger.refresh());
  addEventListener("load", () => ScrollTrigger.refresh());
}
