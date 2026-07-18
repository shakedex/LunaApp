# Luna Web — Visual Design & shadcn Implementation Spec

**Date:** 2026-07-19
**Branch:** `feature/luna-web`
**Status:** Approved via brainstorming — accent = Luna-logo cyan, cat+moon logo used throughout
**Scope:** `web/app` UI only. PDF document design explicitly excluded. No new features.

---

## 1. Summary

Luna Web's UI is functionally complete for scan → process → results → export but visually
barebones: default zinc shadcn tokens, a single centered column, hand-rolled stat lists and
skeletons, and a half-finished shadcn setup (one hand-copied Button, no `components.json`).

This pass does two things:

1. **Implements shadcn/ui properly** — CLI-initialized (`components.json`), CLI-managed
   components, all styling behind semantic tokens.
2. **Establishes the visual identity** — a "Cinema Dark" direction: dark-only v1, layered
   near-black surfaces, hairline borders, a single **brand-cyan** accent sampled from the
   Luna logo, Geist + Geist Mono, thumbnails as the hero content.
3. **Brings the real Luna brand into the web app** — the canonical cat-and-crescent-moon
   logo (`Assets/luna-logo*`) is used in the header, landing hero, capability gate, favicon,
   and social preview (§4b), replacing the abstract placeholder favicon and the generic
   lucide glyph.

Chosen in brainstorming: **Cinema Dark** direction, **dark-only v1**, **full shell +
restyle** scope, **cat+moon logo with the UI accent shifted to the logo's cyan**.

---

## 2. Design tokens (`app/src/index.css`)

All values live behind the existing semantic shadcn variables (`--background`, `--card`,
`--primary`, …). Light mode is out of scope but remains a purely additive later pass
because no component references raw colors.

| Token | Value (oklch) | Approx | Role |
|---|---|---|---|
| `--background` | `oklch(0.13 0.005 285)` | `#0a0a0c` | page base — never pure black |
| `--card` | `oklch(0.17 0.005 285)` | `#141416` | elevated surfaces |
| `--popover` | `oklch(0.19 0.005 285)` | — | menus, tooltips |
| `--border` | `oklch(1 0 0 / 8%)` | `rgba(255,255,255,.08)` | hairlines everywhere |
| `--input` | `oklch(1 0 0 / 14%)` | — | interactive/hover border step |
| `--primary` | `oklch(0.79 0.09 226)` | `#78C6EA` | the single accent — **sampled from the logo crescent** |
| `--primary-foreground` | `oklch(0.2 0.02 240)` | `#08131a` | on-accent text (dark, on the light-cyan fill) |
| `--accent-foreground` / links | `oklch(0.84 0.09 226)` | `#9AD6F2` | brighter cyan for accent text on dark surfaces (keeps ≥4.5:1) |
| `--foreground` | `oklch(0.93 0.003 285)` | `#EDEDEF` | primary text |
| `--muted-foreground` | `oklch(0.64 0.01 285)` | `#8A8F98` | secondary text (≥4.5:1 on bg) |
| `--destructive` | soft red, `oklch(0.65 0.2 25)` band | — | failures |
| `--ring` | `= --primary` | `#78C6EA` | focus rings |
| `--radius` | `0.5rem` | — | tightened for data density |

**Accent sourcing:** `#78C6EA` is the dominant crescent-moon cyan sampled directly from
`Assets/luna-logo-lg.webp`, so the UI accent and the brand mark are the same color family.
The logo's brighter internal glow tone (`~#47BFFF`) is reserved for the static CTA glow.
Because the accent fill is light, primary buttons use **dark text on cyan** (not white),
which reads cleanly on near-black and passes AA.

Additional decisions:

- **Depth model:** surface steps + hairline borders, **no drop shadows**. The one
  exception: a faint static accent glow (`box-shadow: 0 0 24px` at ~25% of the logo glow
  cyan `#47BFFF`) behind the single primary CTA per screen. Nowhere else.
- **Status colors:** amber (RAW/unsupported notices), soft red (failed), green (done) —
  always paired with icon or text, never color alone.
- **Typography:** Geist Variable (existing) for UI. Add `@fontsource-variable/geist-mono`
  for timecode, file paths, and metadata values. `tabular-nums` on all numeric data.
  Type scale: 12/14 for data rows, 16 base, 20/24/30 headings.
- `.dark` class applied statically on `<html>` (in `index.html`); `color-scheme: dark`.
- Chart tokens re-derived from the accent + neutrals (used later if charts appear).

---

## 3. shadcn implementation

- **Init:** create `app/components.json` for the Base UI registry (shadcn 4.x
  `base-nova`, matching the existing Button primitive and the Luna Web spec §7).
  CLI-first per project workflow rules — `bun x shadcn add <component>` for everything;
  no hand-copied component code going forward.
- **Components added:** `badge`, `card`, `table`, `input`, `label`, `field`, `progress`,
  `skeleton`, `separator`, `tooltip`, `alert`, `empty`, `spinner`, `kbd`.
  (`button` already exists; it remains CLI-owned.)
- **Migration:** hand-rolled UI is replaced — stat `<dl>` blocks → stat tile composition
  on `Card`, ad-hoc `animate-pulse` divs → `Skeleton`, status `<span>`s → `Badge`
  variants, bare bordered `<ul>`s → `Table`/list-on-`Card` patterns, cover form fields →
  `Field`/`Label`/`Input`.
- **Rule:** no raw hex/oklch inside components; semantic Tailwind classes only.

---

## 4. App shell

- **Header:** slim (56px), sticky, `bg-background/80` + `backdrop-blur`, hairline bottom
  border. Left: the **Luna logo mark** (cat+moon, `luna-logo.webp`, rendered at 24–28px
  height) + **Luna** wordmark in Geist. Right: **Docs** (plain anchor to `/docs`) and the
  app version, muted. Nothing else — no taglines, no badges.
- **Layout:** content container `max-w-5xl`, consistent page padding, phase content
  renders inside the shell.
- **Capability gate:** restyled onto the same tokens (currently hardcoded zinc), same
  layout as today, now led by the **logo mark** above the heading: logo + "Luna Web needs
  a Chromium browser" + explanation.
- **Metadata / SEO (replaces in-UI privacy messaging):** the local-only/privacy story is
  conveyed through proper page metadata in `app/index.html` — `<title>`
  ("Luna — local camera reports in your browser" band), meta description stating
  client-side processing factually, OG/twitter tags, `theme-color` matching
  `--background`, and the logo-based favicon + social image (§4b). No marketing pills or
  slogan chips in the app chrome; the landing screen keeps its existing single factual line.

---

## 4b. Brand assets & logo usage

**Source of truth:** the canonical Luna mark is the black-cat-and-cyan-crescent-moon logo.
Masters live at repo root: `Assets/luna-logo.webp` (small), `Assets/luna-logo-lg.webp`
(317×333, transparent), `Assets/luna-logo.ico`, `Assets/luna-logo.icns`, and the layered
`docs/branding/luna-logo.psd`. Both webp files are **VP8X with a real alpha channel**, so
they composite correctly on the near-black UI. The desktop app and the web app now share
one identity.

### 4b.1 Getting the assets into the app
Vite only serves files under `web/app/public/` (or ES-imported). This pass **copies** the
masters into the app rather than referencing repo-root paths:

- `web/app/public/luna-logo.webp` ← `Assets/luna-logo.webp` (in-app mark: header, gate,
  hero).
- `web/app/public/luna-logo-lg.webp` ← `Assets/luna-logo-lg.webp` (hi-DPI hero / OG source).
- `web/app/public/favicon.ico` ← `Assets/luna-logo.ico`, and the `<link rel="icon">` in
  `index.html` is repointed to it. The current abstract violet `favicon.svg` placeholder is
  **removed** from the app (it is not the brand). The unrelated sparkle
  `web/docs/public/favicon.svg` is out of scope (docs site) and left untouched.

> Copying (not importing from `../../..`) keeps the app build self-contained and avoids a
> cross-package path into the .NET tree. If drift becomes a concern later, a build step can
> sync them; for one pass, a copy is the disposable-correct choice.

### 4b.2 Where the logo appears
| Surface | Asset | Size / treatment |
|---|---|---|
| Header wordmark | `luna-logo.webp` | 24–28px tall, left of "Luna", `alt="Luna"` |
| Landing hero | `luna-logo.webp` (or `-lg` for crispness) | 48–64px, centered above the wordmark/value line |
| Capability gate | `luna-logo.webp` | 40–48px, above the heading |
| Favicon / tab | `favicon.ico` (from `luna-logo.ico`) | browser-generated sizes |
| Social preview (OG/Twitter) | new `web/app/public/og.png` | 1200×630, logo centered on the `#0a0a0c` background + "Luna Web" wordmark; PNG for scraper compatibility |
| Report cover default logo | **unchanged** | the cover-form logo upload is user data, not the app mark; not affected here |

### 4b.3 Rendering rules
- Always render as an `<img>` with a descriptive `alt` (`"Luna"`), never as a CSS
  background (keeps it accessible and print-safe).
- Do **not** recolor, add a backing plate, or apply effects — the mark is self-contained
  and already reads on dark. No drop shadow, no glow on the logo itself (the CTA glow is a
  separate element).
- Fixed intrinsic aspect ratio (317×333 ≈ 0.95); set explicit `height` and `width:auto`
  (or the reverse) so it never distorts and reserves layout space (no CLS).
- `loading="eager"` for the header/gate mark (above the fold); the OG image is static.

---

## 5. Screens

### 5.1 Landing (idle)
Centered hero: **Luna logo mark** (48–64px, §4b.2) above the wordmark, the existing
one-line factual description, large accent **Pick folder** button (the screen's glowing
CTA), supported-formats hint in muted mono beneath. Recent sources: compact card list —
folder icon, name, last-used, hover border-step + background lift; stale entries muted with
a re-pick affordance.

### 5.2 Scanning
`Spinner` + live counts (`aria-live` kept), indeterminate `Progress`. Muted, quiet.

### 5.3 Pre-scan summary
`Card` with three stat tiles (Clips / Total size / RAW). RAW warning becomes an amber
`Alert` with icon. Actions: **Process N clips** (primary), **Cancel** (ghost).

### 5.4 Processing
Section header: title + `n/m` counter + determinate `Progress` bar; **Start over** ghost
button. Clip rows: `Skeleton` shimmer thumbnails while decoding, status `Badge`
(queued/processing/failed) right-aligned, metadata cells fill in as they arrive.

### 5.5 Results workspace
- **Stats row:** five stat tiles (Cards / Clips / Duration / Size / RAW) as one `Card`
  strip, numbers in tabular Geist, labels muted small-caps style.
- **Cover form:** two-column `Field` grid on a `Card`, logo upload styled as a drop-well.
- **Reels:** each reel a section with a sticky sub-header (reel name + `clip count · size`
  muted); clip list on a `Card` with hairline row separators.
- **Clip row:** 3-frame thumbnail strip (rounded, hairline border, subtle hover lift
  ~`scale-[1.02]` + border step), filename (truncating) + start-timecode `Badge` in Geist
  Mono, metadata columns (resolution/codec/fps/duration/size) in muted mono,
  failed thumbs as placeholder tiles with outcome text.
- **Export toolbar:** **PDF** primary (accent), **CSV** outline, disabled-with-spinner
  while generating.
- **RAW section:** muted list on `Card`, amber accent icon per row.

### 5.6 Error / empty states
`Empty` pattern: icon, one-line cause, recovery action (retry / back). Scan errors and
the processed-but-zero-clips case both use it.

---

## 6. Motion & accessibility

- Phase transitions: 150–250ms fade/slide-up (`tw-animate-css` utilities).
- Skeleton shimmer during decode; hover transitions 150ms; no animation over 300ms.
- `prefers-reduced-motion`: transitions collapse to instant.
- Focus: visible accent rings on every interactive element (shadcn defaults kept).
- Contrast verified against the new dark surfaces: body ≥4.5:1, muted ≥4.5:1,
  amber/red/green status text ≥4.5:1 on their backgrounds. The cyan accent is used two
  ways: **dark text on the light-cyan CTA fill** (AA), and the **brighter cyan `#9AD6F2`
  for accent text on dark surfaces** (AA) — never the fill-cyan as small text on dark.
- All UI icons lucide SVG, one stroke width (no emoji glyphs anywhere). The Luna logo is
  the only raster brand asset; it carries a descriptive `alt`.

---

## 7. Out of scope

- Light mode (tokens stay semantic; additive later).
- PDF document design (user-excluded). The report-cover **user logo upload** is likewise
  untouched — that is user data, distinct from the Luna app mark.
- New routes/features (settings, activity, credits screens arrive with their milestones
  and will inherit this system).
- Mobile/touch layout (desktop Chromium target per product spec).
- The docs site (`web/docs`) branding, including its placeholder sparkle favicon.

---

## 8. Verification

- `bun run lint`, `bun run typecheck` clean.
- Manual visual pass in Chromium at 1280 and 1440 wide: every phase (idle → scanning →
  summary → processing → results → error), hover/focus/disabled states, reduced-motion.
- Contrast spot-checks on muted text, the cyan accent (both pairings), and status colors
  against the new surfaces.
- Logo assets: confirm the copied `luna-logo.webp` and `favicon.ico` render crisply and
  transparently on the dark header/gate/hero, the browser tab shows the cat+moon favicon,
  and the OG image previews correctly (e.g. via a link-preview/debugger check).
