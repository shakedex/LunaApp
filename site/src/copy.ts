// Every visible string on the site, in one place. All of it is a draft for
// Shaked to rewrite. Rules: labels and facts only; no explanations, no
// taglines, no "No X, no Y", no "X. Y." pairs, no dashes as punctuation.

export const REPO_URL = "https://github.com/shakedex/LunaApp";
export const RELEASES_URL = `${REPO_URL}/releases`;
export const LATEST_URL = `${RELEASES_URL}/latest`;

/** The release the static page names before the live lookup fills in. */
export const KNOWN_VERSION = "2026.10.0";

/** The sample report's file names, as Luna writes them. */
export const REPORT_FILES = {
  pdf: "Luna Report 2026-10-21 1942.pdf",
  html: "Luna Report 2026-10-21 1942.html",
  csv: "Luna Report 2026-10-21 1942.csv",
};

export const copy = {
  meta: {
    title: "Luna",
    description: "Camera reports from camera originals. PDF, HTML and CSV. Free, for Windows and macOS.",
  },

  nav: {
    how: "How it works",
    report: "The report",
    formats: "Formats",
    download: "Download",
    github: "GitHub",
  },

  hero: {
    title: "Camera reports from camera originals.",
    lede: "Reads a card or a whole shoot drive and writes the camera report as PDF, HTML and CSV.",
    alsoFor: "Also for",
    windowAlt: "Luna's window: the engraved Moon fills as the clips are read.",
  },

  /** The scroll sequence: three steps, and the window's own lines for each. */
  stage: {
    steps: ["Drop a camera folder", "Luna reads every clip", "Report ready"],
    window: {
      idle: ["Drop a camera folder", "or click anywhere to choose one"],
      looking: "Looking for clips",
      clips: (done: number, total: number) => `${done} of ${total} clips`,
      source: "E:\\NIGHT_SHIFT_DAY12",
      ready: "Report ready",
      summary: "25 clips, no problems",
    },
    stepAlts: [
      "Luna's window waiting for a folder, with a thin crescent Moon.",
      "Luna's window reading clip 17 of 25, the Moon two thirds lit.",
      "Luna's window with the full Moon, Report ready, and the Open PDF button.",
    ],
  },

  report: {
    title: "The report",
    paper: "Paper",
    dark: "Dark",
    pageAlt: "The first page of a Luna camera report: production header, totals, Camera A, reel A003 and its clips.",
    detailAlt: "One clip row of the report: three frames, then timecode, camera, lens and exposure fields.",
    pagesAlt: (n: number) => `Page ${n} of the sample report`,
    files: [
      { label: "PDF", name: REPORT_FILES.pdf },
      { label: "HTML", name: REPORT_FILES.html },
      { label: "CSV", name: REPORT_FILES.csv },
    ],
  },

  formats: {
    title: "Formats",
    lede: "On Windows and macOS.",
    rows: [
      { maker: "ARRI", formats: ["ARRIRAW", "ARRICORE", "ProRes"], ext: [".ari", ".arx", ".mxf", ".mov"] },
      { maker: "Sony", formats: ["X-OCN", "XAVC"], ext: [".mxf"] },
      { maker: "RED", formats: ["R3D"], ext: [".r3d"] },
      { maker: "Blackmagic Design", formats: ["Blackmagic RAW"], ext: [".braw"] },
      { maker: "Canon", formats: ["Cinema RAW Light", "XF-AVC", "XF-HEVC"], ext: [".crm", ".mxf", ".mp4"] },
      { maker: "Nikon", formats: ["N-RAW", "R3D NE"], ext: [".nev", ".r3d"] },
      { maker: "Apple", formats: ["ProRes", "ProRes RAW"], ext: [".mov"] },
      { maker: "Other cameras", formats: ["H.264", "HEVC"], ext: [".mov", ".mp4"] },
    ],
    sidecarsLabel: "Sidecars",
    sidecars: "Sony and Canon XML, RED RMD, Blackmagic RAW sidecars, ALE",
  },

  download: {
    title: "Download",
    windows: {
      name: "Windows",
      button: "Download for Windows",
      file: (v: string) => `Luna-${v}-windows-x64-setup.exe`,
      requires: "Windows 10 or 11, x64",
    },
    macos: {
      name: "macOS",
      button: "Download for macOS",
      file: (v: string) => `Luna-${v}-macos.dmg`,
      requires: "macOS 26 or later, Apple Silicon",
    },
    version: (v: string) => `Version ${v}`,
    free: "Free. MIT licence.",
    allReleases: "All releases",
    source: "Source on GitHub",
  },

  footer: {
    links: [
      { label: "GitHub", href: REPO_URL },
      { label: "Releases", href: RELEASES_URL },
      { label: "Licence", href: `${REPO_URL}/blob/master/LICENSE` },
      { label: "Disclaimer", href: `${REPO_URL}/blob/master/DISCLAIMER.md` },
    ],
    legal:
      "ARRI, Sony, RED, Blackmagic Design, Canon, Nikon and Apple ProRes are trademarks of their owners. Luna is not affiliated with or endorsed by any of them.",
  },

  notFound: {
    title: "Page not found",
    back: "Home",
  },
};
