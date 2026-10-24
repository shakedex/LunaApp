// The pinned "How it works" sequence, scrubbed by scroll, and where the one
// Moon sits on the page: the hero slot, then the app window's Moon spot, then
// the footer. The Moon replays a run inside the window: waiting, reading
// (17 of 25, as the captured screen says), report ready.

import type { gsap as Gsap } from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import layout from "../../public/img/layout.json";
import { lerpRect, windowMoonRect, type Rect } from "./dock";
import type { MoonState, Placement } from "./main";

const WAITING = 0.08; // the app's lit share while it waits
const RUNNING = 0.68; // the captured running screen: 17 of 25 clips

interface Setup {
  gsap: typeof Gsap;
  state: MoonState;
  intro: gsap.core.Tween;
  heroRect: () => Rect;
}

/** Builds the scroll timeline and returns where the Moon should be this frame. */
export function setupScroll({ gsap, state, intro, heroRect }: Setup): () => Placement {
  gsap.registerPlugin(ScrollTrigger);
  document.documentElement.classList.add("motion");

  const q = (s: string) => document.querySelector(s) as HTMLElement;
  const win = q("#app-window");
  const footer = q("#footer-moon-slot");
  const stage = (i: number) => q(`.how-stage[data-stage="${i}"]`);
  const strips = gsap.utils.toArray<HTMLElement>(".strip");

  const tl = gsap.timeline({
    defaults: { ease: "none" },
    scrollTrigger: {
      trigger: ".pinned",
      start: "top top",
      end: "+=300%",
      pin: ".pinned .stage",
      scrub: 1,
      invalidateOnRefresh: true,
      onUpdate: () => {
        // Scrolling in takes over from the hero intro.
        if (intro.isActive()) intro.progress(1);
      },
    },
  });

  // Waiting: the Moon flies into the window and wanes to the app's crescent; cards drop in.
  tl.fromTo(state, { dock: 0, lit: 1, progress: 1 }, { dock: 1, lit: WAITING, progress: 0, duration: 0.5, ease: "power2.inOut", immediateRender: false }, 0)
    .fromTo(".card", { y: -260, opacity: 0 }, { y: 0, opacity: 1, stagger: 0.08, duration: 0.4, ease: "power2.out" }, 0.35)
    .to(".card", { y: 120, scale: 0.4, opacity: 0, stagger: 0.05, duration: 0.25, ease: "power2.in" }, 0.8)

    // Reading: the screen changes, the Moon waxes, frame strips leave the window.
    .to(stage(0), { opacity: 0, duration: 0.12 }, 1)
    .to(stage(1), { opacity: 1, duration: 0.12 }, 1.08)
    .to(".layer-running", { opacity: 1, duration: 0.1 }, 1)
    .to(state, { lit: RUNNING, progress: RUNNING, duration: 0.8 }, 1.1)
    .fromTo(
      strips,
      { opacity: 0, scale: 0.3, x: 0, y: 0 },
      { opacity: 1, scale: 1, x: (i) => [-0.62, -0.7, -0.62][i] * win.offsetWidth, y: (i) => (i - 1) * 0.2 * win.offsetHeight, stagger: 0.12, duration: 0.45, ease: "power2.out" },
      1.35,
    )

    // Report ready: full Moon, the strips settle into the report page.
    .to(stage(1), { opacity: 0, duration: 0.12 }, 2)
    .to(stage(2), { opacity: 1, duration: 0.12 }, 2.08)
    .to(".layer-done", { opacity: 1, duration: 0.1 }, 2)
    .to(state, { lit: 1, progress: 1, duration: 0.4 }, 2)
    .to(strips, { x: (i) => 0.1 * win.offsetWidth, y: (i) => (0.05 + i * 0.09) * win.offsetHeight, scale: 0.42, opacity: 0, stagger: 0.06, duration: 0.45, ease: "power2.inOut" }, 2.15)
    .to(win, { xPercent: -12, scale: 0.92, duration: 0.5, ease: "power2.inOut" }, 2.15)
    .to(state, { shown: 0, duration: 0.2 }, 2.38)
    .fromTo(".pinned .page", { opacity: 0, y: 120, scale: 0.86, rotate: 0 }, { opacity: 1, y: 0, scale: 1, rotate: -1.5, duration: 0.55, ease: "power2.out" }, 2.3)
    .to({}, { duration: 0.3 });

  // The pinned layout is taller than the still one: keep a reader who arrived at an anchor there.
  const target = location.hash && document.querySelector(location.hash);
  if (target) {
    const jump = () => {
      ScrollTrigger.removeEventListener("refresh", jump);
      target.scrollIntoView();
    };
    ScrollTrigger.addEventListener("refresh", jump);
    ScrollTrigger.refresh();
  }

  const windowRect = () => windowMoonRect(win.getBoundingClientRect(), layout);
  const footerRect = (): Rect => {
    const r = footer.getBoundingClientRect();
    return { x: r.left, y: r.top, size: r.width };
  };

  return () => {
    if (state.dock < 1) return { rect: lerpRect(heroRect(), windowRect(), state.dock), opacity: state.shown };
    const f = footerRect();
    // Past the sequence the Moon stays hidden behind the report, and comes back in the footer.
    return f.y < window.innerHeight ? { rect: f, opacity: 1 } : { rect: windowRect(), opacity: state.shown };
  };
}
