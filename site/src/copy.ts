// Every visible string on the site. All of it is a draft for Shaked to rewrite.
// Rules: facts and labels only; no taglines, no "No X, no Y", no "X. Y." pairs, no em dashes.
// Vendor wording: no RED name or marks (R3D is a format), "compatible with Blackmagic RAW",
// no claim of Apple certification for ProRes.

export const RELEASES_LATEST = "https://github.com/shakedex/LunaApp/releases/latest";
export const REPO = "https://github.com/shakedex/LunaApp";

/** Formats still missing from the macOS build (docs/macos.md items 2 to 5). */
const WINDOWS_ONLY = "Windows only for now";

export const copy = {
  meta: {
    title: "Luna, camera reports",
    description: "Luna reads camera cards and writes a camera report as PDF, HTML and CSV. Windows and macOS.",
  },
  nav: {
    how: "How it works",
    report: "Report",
    cameras: "Formats",
    github: "GitHub",
    download: "Download",
  },
  hero: {
    title: "Camera reports from camera originals",
    lede: "Choose a card or a shoot drive. Luna writes a PDF, HTML and CSV report of every clip on it.",
    alsoOn: "Also for",
    moonLabel: "Luna's progress moon",
    clips: (done: number, total: number) => `${done} of ${total} clips`,
    reading: (reel: string) => `Reading ${reel}`,
    ready: "Report ready",
    readySub: "PDF, HTML and CSV",
  },
  how: {
    title: "How it works",
    stages: [
      {
        title: "Choose a folder",
        body: "A single card, or a drive holding cards from several cameras.",
        alt: "Luna waiting for a camera folder",
      },
      {
        title: "Luna reads each clip",
        body: "Timecode, lens, exposure settings and three frames from every clip, decoded on your computer.",
        alt: "Luna reading clip 17 of 25",
      },
      {
        title: "The report",
        body: "Clips grouped by camera and reel. Clips with problems are counted at the top of the report.",
        alt: "Luna with the report ready",
      },
    ],
  },
  report: {
    title: "The report",
    body: "The sample below is 11 clips from two cameras on one shoot day. Scroll it, or switch it to the dark version.",
    frameTitle: "Sample Luna camera report",
    light: "Light",
    dark: "Dark",
    outputs: [
      { name: "PDF", detail: "For printing and email" },
      { name: "HTML", detail: "One file, opens in a browser" },
      { name: "CSV", detail: "One row per clip, every field" },
    ],
  },
  formats: {
    title: "Formats",
    groups: [
      { name: "ARRI", formats: "ARRIRAW, ARRICORE, ProRes in MXF and MOV" },
      { name: "Sony", formats: "X-OCN, XAVC" },
      { name: "Canon", formats: "Cinema RAW Light, XF-AVC, XF-HEVC" },
      { name: "Nikon", formats: "N-RAW, R3D NE", note: WINDOWS_ONLY },
      { name: "R3D", formats: ".R3D files", note: WINDOWS_ONLY },
      { name: "Blackmagic RAW", formats: "Luna is compatible with Blackmagic RAW (.braw)", note: WINDOWS_ONLY },
      { name: "ProRes RAW", formats: "MOV", note: WINDOWS_ONLY },
      { name: "H.264 and HEVC", formats: "MOV and MP4 from other cameras", note: WINDOWS_ONLY },
    ],
  },
  specs: {
    title: "Specifications",
    rows: [
      { label: "Windows", value: "Windows 10 and 11, x64" },
      { label: "macOS", value: "macOS 26 or later, Apple Silicon" },
      { label: "Report files", value: "PDF, HTML, CSV" },
      { label: "Internet", value: "Not used to make a report" },
      { label: "Command line", value: "luna-cli, installed with the app" },
      { label: "Licence", value: "Source code under MIT. The installers also include closed decoders and third-party SDKs." },
      { label: "Price", value: "Free" },
    ],
  },
  footer: {
    links: [
      { label: "GitHub", href: REPO },
      { label: "Releases", href: "https://github.com/shakedex/LunaApp/releases" },
      { label: "License", href: "https://github.com/shakedex/LunaApp/blob/master/LICENSE" },
    ],
    legal:
      "ARRI, Sony, Canon, Nikon, Blackmagic RAW and ProRes are trademarks of their owners. Luna is not affiliated with or endorsed by them.",
  },
  notFound: {
    title: "Page not found",
    body: "This page does not exist.",
    back: "Go to the home page",
  },
};
