// The pinned sequence at the top of the page. Scroll drives one timeline:
// the intro gives way to the three steps; a pointer drags a folder onto the
// window; the window reads, the live Moon filling in step; the report is
// ready; the pointer clicks Open PDF and the first page rises out of the
// window. The Moon turns slowly on its own the whole time.
import { gsap } from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import { copy } from "../copy";
import { detectPlatform, downloadLabels } from "../lib/platform";
import type { Moon } from "../moon/scene";

interface Rows {
  moon: { cx: number; cy: number; box: number };
  titleTop: number;
  subTop: number;
  buttonTop: number;
}
interface Layout {
  width: number;
  height: number;
  "hero-idle": Rows;
  "hero-running": Rows;
  "hero-done": Rows;
}

const TOTAL = 25;
const TRAVEL = 4600; // px of scroll for the whole sequence
const SPIN_PERIOD = 90; // seconds per turn, the app's idle speed

// The beats, as shares of the scroll.
const T = {
  stepsIn: 0.04,
  dragStart: 0.1,
  drop: 0.3, // the folder lands: the window starts reading
  readEnd: 0.72, // the Moon is full
  pointerBack: 0.8,
  click: 0.88, // Open PDF
  pageUp: 0.9,
};

function labelDownloads() {
  const platform = detectPlatform(navigator.userAgent);
  const labels = downloadLabels(platform);
  document.documentElement.dataset.os = platform === "macos" ? "macos" : "windows";
  document.querySelectorAll("[data-download-label]").forEach((el) => (el.textContent = labels.primary));
  document.querySelectorAll("[data-download-other]").forEach((el) => (el.textContent = labels.secondary));
}

export function runStage() {
  labelDownloads();
  const ids = [
    "how",
    "stage-pin",
    "stage-intro",
    "stage-steps",
    "stage-window",
    "stage-moon",
    "stage-line-1",
    "stage-line-2",
    "stage-page",
    "stage-folder",
    "stage-pointer",
  ] as const;
  const el = Object.fromEntries(ids.map((id) => [id, document.getElementById(id)])) as Record<
    (typeof ids)[number],
    HTMLElement | null
  >;
  if (Object.values(el).some((e) => !e)) return;

  const wide = matchMedia("(min-width: 1000px)").matches;
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (!wide || reduce) return;

  const stage = el.how!;
  const win = el["stage-window"]!;
  const canvas = el["stage-moon"] as HTMLCanvasElement;
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
      for (const id of ["stage-moon", "stage-line-1", "stage-line-2", "stage-folder", "stage-pointer"] as const) {
        el[id]!.hidden = false;
      }
      document.documentElement.classList.add("motion");
      build({
        layout,
        layers,
        moon,
        stage,
        win,
        canvas,
        pin: el["stage-pin"]!,
        intro: el["stage-intro"]!,
        steps: el["stage-steps"]!,
        line1: el["stage-line-1"]!,
        line2: el["stage-line-2"]!,
        page: el["stage-page"]!,
        folder: el["stage-folder"]!,
        pointer: el["stage-pointer"]!,
      });
    })
    .catch((e) => console.warn("Luna: the live Moon failed, the plain page stays", e));
}

interface Parts {
  layout: Layout;
  layers: Record<"idle" | "running" | "done", HTMLImageElement>;
  moon: Moon;
  stage: HTMLElement;
  win: HTMLElement;
  canvas: HTMLCanvasElement;
  pin: HTMLElement;
  intro: HTMLElement;
  steps: HTMLElement;
  line1: HTMLElement;
  line2: HTMLElement;
  page: HTMLElement;
  folder: HTMLElement;
  pointer: HTMLElement;
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

  // Window coordinates, as shares of the window, for the pointer's targets.
  const moonCx = pct(layout["hero-idle"].moon.cx, layout.width);
  const moonCy = pct(layout["hero-idle"].moon.cy, layout.height);
  // The Open PDF button: the done layout's button row, left of centre.
  const buttonCx = 42.4;
  const buttonCy = pct(layout["hero-done"].buttonTop, layout.height) + 2.6;
  const at = (x: number, y: number) => ({ left: `${x}%`, top: `${y}%` });

  // What the window says, and how lit the Moon is.
  const state = { phase: "idle" as "idle" | "running" | "done", progress: 0, hover: false };
  const applyText = () => {
    if (state.phase === "idle") {
      p.line1.textContent = state.hover ? w.release : w.idle[0];
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

  // Starting places: the page below the viewport, the folder and pointer
  // just outside the window's left edge.
  gsap.set(p.page, { xPercent: -50, yPercent: -50, y: () => innerHeight * 1.2 });
  gsap.set(p.folder, { xPercent: -50, yPercent: -50, ...at(-12, 78), opacity: 1 });
  gsap.set(p.pointer, { ...at(-10, 82), opacity: 0 });

  const tl = gsap.timeline({
    defaults: { ease: "none" },
    scrollTrigger: {
      trigger: p.stage,
      start: "top top",
      end: `+=${TRAVEL}`,
      pin: p.pin,
      scrub: 0.6,
      anticipatePin: 1,
      invalidateOnRefresh: true,
      onUpdate: (self) => {
        const t = self.progress;
        setPhase(t < T.drop ? "idle" : t < T.readEnd ? "running" : "done");
        state.hover = t > T.dragStart + 0.1 && t < T.drop;
        applyText();
      },
    },
  });

  tl
    // The intro gives way to the steps.
    .to(p.intro, { autoAlpha: 0, y: -24, duration: 0.08, ease: "power2.in" }, 0)
    .to(p.steps, { autoAlpha: 1, duration: 0.08 }, T.stepsIn)
    // The pointer picks the folder up outside the window and drags it to the Moon.
    .to(p.pointer, { opacity: 1, duration: 0.03 }, T.dragStart)
    .to(p.pointer, { ...at(moonCx + 0.5, moonCy - 2.5), duration: T.drop - T.dragStart, ease: "power1.inOut" }, T.dragStart)
    .to(p.folder, { ...at(moonCx, moonCy), duration: T.drop - T.dragStart, ease: "power1.inOut" }, T.dragStart)
    // Dropped: the folder goes into the Moon and the window starts reading.
    .to(p.folder, { scale: 0.4, opacity: 0, duration: 0.04, ease: "power2.in" }, T.drop - 0.02)
    .set(layers.idle, { opacity: 0 }, T.drop)
    .set(layers.running, { opacity: 1 }, T.drop)
    .set(pos, { ...running, onUpdate: applyPos }, T.drop)
    .to(p.pointer, { ...at(moonCx + 22, moonCy + 30), opacity: 0, duration: 0.06, ease: "power1.out" }, T.drop)
    // The clips are read: the Moon fills.
    .to(state, { progress: 1, duration: T.readEnd - T.drop - 0.02 }, T.drop + 0.02)
    // Done.
    .set(layers.running, { opacity: 0 }, T.readEnd)
    .set(layers.done, { opacity: 1 }, T.readEnd)
    .set(pos, { ...done, onUpdate: applyPos }, T.readEnd)
    // The pointer comes back and opens the PDF.
    .set(p.pointer, { ...at(moonCx + 30, moonCy + 44) }, T.pointerBack)
    .to(p.pointer, { opacity: 1, duration: 0.02 }, T.pointerBack)
    .to(p.pointer, { ...at(buttonCx, buttonCy), duration: T.click - T.pointerBack - 0.01, ease: "power1.inOut" }, T.pointerBack)
    .to(p.pointer, { scale: 0.82, duration: 0.01 }, T.click)
    .to(p.pointer, { scale: 1, duration: 0.01 }, T.click + 0.01)
    // The first page rises out of the window.
    .to(p.page, { y: 0, duration: 1 - T.pageUp, ease: "power2.out" }, T.pageUp)
    .to(p.win, { opacity: 0.12, scale: 0.94, duration: 0.08 }, T.pageUp + 0.01)
    .to(p.pointer, { opacity: 0, duration: 0.02 }, T.pageUp + 0.01)
    .to(p.steps, { autoAlpha: 0, duration: 0.06 }, T.pageUp + 0.03);

  // Fonts and images change the layout as they arrive.
  document.fonts?.ready.then(() => ScrollTrigger.refresh());
  addEventListener("load", () => ScrollTrigger.refresh());
}
