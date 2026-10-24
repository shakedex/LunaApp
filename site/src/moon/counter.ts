import { copy } from "../copy";

/** The sample report's reels in reading order, with the last clip count each one reaches. */
const REELS: [string, number][] = [
  ["A003", 9],
  ["A004", 18],
  ["B002", Infinity],
];

/** The clip counter under the hero Moon at `progress` (0 to 1) of `total` clips. */
export function counterText(progress: number, total: number): { count: string; sub: string } {
  const done = Math.round(Math.min(1, Math.max(0, progress)) * total);
  if (done >= total) return { count: copy.hero.ready, sub: copy.hero.readySub };
  const reel = REELS.find(([, last]) => done <= last)![0];
  return { count: copy.hero.clips(done, total), sub: copy.hero.reading(reel) };
}
