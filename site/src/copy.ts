// Every visible string on the site, in one place.

export const REPO_URL = "https://github.com/shakedex/LunaApp";
export const RELEASES_URL = `${REPO_URL}/releases`;
export const LATEST_URL = `${RELEASES_URL}/latest`;

/** The release the static page names before the live lookup fills in. */
export const KNOWN_VERSION = "2026.10.0";

export const copy = {
  meta: {
    title: "Luna: beautiful reports for every camera",
    description:
      "Camera reports from ARRIRAW, X-OCN, R3D, Blackmagic RAW, Cinema RAW Light, N-RAW and ProRes RAW originals, as PDF, HTML and CSV. Free for Windows and macOS.",
    ogImageAlt: "Luna: beautiful reports for every camera. The app's window with the full Moon over the first page of a report.",
    category: "MultimediaApplication",
    os: "Windows 10, Windows 11, macOS 26",
  },

  nav: {
    report: "Report",
    formats: "Formats",
    features: "Features",
    download: "Download",
  },

  hero: {
    title: "Beautiful reports for every camera. Free.",
    windowAlt: "Luna's window: the engraved Moon fills as the clips are read.",
  },

  /** The scroll sequence: three steps, and the window's own lines for each. */
  stage: {
    steps: ["Drop a camera folder", "Luna reads every clip", "Report ready"],
    folder: "NIGHT_SHIFT_DAY12",
    window: {
      idle: ["Drop a camera folder", "or click anywhere to choose one"],
      release: "Release to use this folder",
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
    title: "Report",
    paper: "Paper",
    dark: "Dark",
    pageAlt: "The first page of a Luna camera report: production header, totals, Camera A, reel A003 and its clips.",
    detailAlt: "One clip row of the report: three frames, then timecode, camera, lens and exposure fields.",
    pagesAlt: (n: number) => `Page ${n} of the sample report`,
  },

  formats: {
    title: "Formats",
    rows: [
      { maker: "ARRI", logo: "arri", formats: ["ARRIRAW", "ARRICORE", "ProRes"] },
      { maker: "Sony", logo: "sony", formats: ["X-OCN", "XAVC"] },
      { maker: "RED", logo: "red", formats: ["R3D"] },
      { maker: "Blackmagic Design", logo: "blackmagicdesign", formats: ["Blackmagic RAW"] },
      { maker: "Canon", logo: "canon", formats: ["Cinema RAW Light", "XF-AVC", "XF-HEVC"] },
      { maker: "Nikon", logo: "nikon", formats: ["N-RAW", "R3D NE"] },
      { maker: "Apple", logo: "apple", formats: ["ProRes", "ProRes RAW"] },
      { maker: "Other cameras", logo: null, formats: ["H.264", "HEVC"] },
    ],
  },

  features: {
    title: "Features",
    note: { text: "* New formats and codecs arrive with updates. Missing one?", link: "Ask on GitHub" },
    script: {
      title: "Script it",
      text: "Make reports from a terminal or a script, or let AI agents make them over MCP.",
      commands: ["luna-cli report D:\\DAY12", "luna-cli inspect D:\\DAY12", "luna-cli mcp"],
    },
    items: [
      {
        icon: "aperture",
        title: "Every raw, read natively",
        text: "ARRIRAW, ARRICORE, X-OCN, R3D, BRAW, Cinema RAW Light, N-RAW, ProRes RAW and more*",
      },
      { icon: "file", title: "Paper or dark", text: "PDF, HTML and CSV, in the Paper or Dark theme." },
      { icon: "drives", title: "Drop the whole drive", text: "A day's worth of cards in one go, sorted by camera and reel." },
      { icon: "card", title: "Your logo, your fields", text: "Company logo and your own production fields on every page." },
      { icon: "image", title: "Stills that look right", text: "Raw and log frames are converted to Rec.709 for the thumbnails." },
      { icon: "desktop", title: "Windows and macOS, free", text: "Every format reads on both. MIT licence." },
    ],
  },

  download: {
    title: "Download",
    windows: {
      name: "Windows",
      button: "Download for Windows",
      requires: "Windows 10 or 11, x64",
    },
    macos: {
      name: "macOS",
      button: "Download for macOS",
      requires: "macOS 26 or later, Apple Silicon",
    },
    version: (v: string) => `Version ${v}`,
    free: "Free, MIT licence",
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
