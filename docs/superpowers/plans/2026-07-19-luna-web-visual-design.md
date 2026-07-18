# Luna Web Visual Design + shadcn Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give Luna Web a "Cinema Dark" visual identity built on properly-installed shadcn/ui, with the real Luna cat+moon logo throughout and a UI accent sampled from that logo's cyan — no new features, UI only.

**Architecture:** All work is in `web/app`. First stand up shadcn (`components.json` + CLI-added components), then rewrite the design tokens (`index.css`) to a dark-only cinema palette, bring the brand logo + SEO metadata into the app, add a slim app shell, and restyle each phase screen (capability gate → landing → scanning → summary → processing → results) onto shadcn primitives + the tokens. Nothing touches `packages/core`, the workers, the decode/scan logic, the PDF document, or the .NET desktop app.

**Tech Stack:** React 19, TanStack Router/Store/Form, Tailwind CSS v4 (`@tailwindcss/vite`), shadcn/ui on Base UI (`@base-ui/react`, `base-nova` style), lucide-react, Geist + Geist Mono (fontsource), Vite 8, Bun, Biome.

## Global Constraints

Copied verbatim from the spec (`docs/superpowers/specs/2026-07-19-luna-web-visual-design.md`) — every task's requirements implicitly include these:

- **Scope:** `web/app` UI only. Do **not** touch `packages/core`, `web/docs`, the workers, decode/scan logic, the PDF document (`pdf-document.tsx`/`pdf-*`), or the repo-root .NET app.
- **Dark-only v1.** Ship dark only. Keep everything behind semantic shadcn tokens so light mode stays a purely additive later pass — **no raw hex/oklch inside components**, semantic Tailwind classes only.
- **Accent = Luna-logo cyan.** `--primary` = `oklch(0.79 0.09 226)` (`#78C6EA`, sampled from the logo crescent). Primary buttons use **dark text on the cyan fill** (`--primary-foreground` = `oklch(0.2 0.02 240)`). Accent text on dark surfaces uses the brighter `#9AD6F2`, never the fill-cyan as small text on dark. CTA glow uses the logo glow cyan `#47BFFF` at ~25%.
- **Surfaces never pure black.** background `oklch(0.13 0.005 285)`, card `oklch(0.17 0.005 285)`. Depth = surface steps + `rgba(255,255,255,.08)` hairline borders, **no drop shadows** (the single CTA glow is the only exception).
- **Radius** tightened to `0.5rem`.
- **Icons:** lucide SVG only, one stroke width, **no emoji/text glyphs as icons** anywhere (this includes replacing the existing `✕` in `recent-list.tsx`). The Luna logo is the only raster brand asset and always carries a descriptive `alt`.
- **Logo:** canonical mark is the cat+crescent-moon (`Assets/luna-logo*`). Copy masters into `web/app/public/`; never reference the repo-root `Assets/` path from app code. Render as `<img alt="Luna">`, never recolored, never as a CSS background, with fixed intrinsic aspect ratio (317×333) to avoid CLS.
- **Motion:** 150–300ms transitions, honor `prefers-reduced-motion`, no animation >300ms except the static (non-animated) CTA glow.
- **Contrast:** body ≥4.5:1, muted ≥4.5:1, status text ≥4.5:1 on its surface.

### Testing note (read before every task)

Per spec §17, Luna Web has **no automated UI/e2e tests** — this is an explicit project decision and there is no UI test runner installed. `bun test` covers only `packages/core`, which this plan does not touch. Therefore the "test cycle" for every task in this plan is:

1. `cd web && bun run typecheck` → expect **no errors**.
2. `cd web && bun run lint` → expect **no errors** (Biome).
3. **Manual visual check** in the dev server (`cd web && bun --filter @luna-web/app dev`, open the printed `localhost` URL in Chrome) — each task states exactly what to look at.

Do not invent Vitest/Playwright tests; the project has deliberately excluded them. "Run the test" below always means the three steps above.

---

## File map

**Create:**
- `web/app/components.json` — shadcn config (Task 1)
- `web/app/src/components/ui/*` — CLI-generated shadcn components (Task 1)
- `web/app/public/luna-logo.webp`, `web/app/public/luna-logo-lg.webp`, `web/app/public/favicon.ico`, `web/app/public/og.png` — brand assets (Task 3)
- `web/app/src/components/logo.tsx` — the `<Logo>` mark component (Task 4)
- `web/app/src/components/app-shell.tsx` — header + page container (Task 4)
- `web/app/src/components/stat-tile.tsx` — shared stat tile (Task 6)
- `web/app/src/vite-env.d.ts` — `__APP_VERSION__` global decl (Task 4)

**Modify:**
- `web/app/src/index.css` — token rewrite + mono font (Task 2)
- `web/app/index.html` — `<html class="dark">`, favicon, SEO/OG meta (Task 2 + Task 3)
- `web/app/vite.config.ts` — inject `__APP_VERSION__` (Task 4)
- `web/app/src/routes/__root.tsx` — wrap in shell (Task 4)
- `web/app/src/components/capability-gate.tsx` — restyle + logo (Task 5)
- `web/app/src/features/scan/clip-row.tsx` — restyle (Task 6)
- `web/app/src/features/scan/scan-screen.tsx` — restyle idle/scanning/summary/processing (Tasks 7–9)
- `web/app/src/features/scan/recent-list.tsx` — restyle + lucide `X` (Task 7)
- `web/app/src/features/report/report-workspace.tsx` — restyle results (Task 10)
- `web/app/src/features/report/cover-form.tsx` — restyle onto Field/Input (Task 10)
- `web/app/src/features/export/export-buttons.tsx` — restyle toolbar (Task 10)
- `web/app/package.json` — new deps (`@fontsource-variable/geist-mono`), added by CLI/`bun add`

---

## Task 1: shadcn foundation (components.json + component set)

Stands up shadcn properly so later tasks have primitives to compose. The project already has one Base-UI Button and `@import "shadcn/tailwind.css"`; this adds `components.json` and the missing components.

**Files:**
- Create: `web/app/components.json`
- Create: `web/app/src/components/ui/{badge,card,table,input,label,field,progress,skeleton,separator,tooltip,alert,empty,spinner,kbd}.tsx` (CLI-generated)

**Interfaces:**
- Produces: shadcn components importable as `@/components/ui/<name>` — `Badge`, `Card`/`CardHeader`/`CardTitle`/`CardContent`/`CardFooter`, `Table`/`TableHeader`/`TableBody`/`TableRow`/`TableHead`/`TableCell`, `Input`, `Label`, `Field`/`FieldLabel`/`FieldGroup`, `Progress`, `Skeleton`, `Separator`, `Tooltip`/`TooltipTrigger`/`TooltipContent`, `Alert`/`AlertTitle`/`AlertDescription`, `Empty`/`EmptyHeader`/`EmptyMedia`/`EmptyTitle`/`EmptyDescription`/`EmptyContent`, `Spinner`, `Kbd`. (Exact export names may vary slightly by shadcn version — Task consumers should confirm against the generated file.)

- [ ] **Step 1: Run shadcn init (non-interactive)**

Run from the app package so paths resolve:

```bash
cd web/app && bun x shadcn init --template vite --base base --yes --pointer
```

Notes:
- `--base base` selects Base UI (matches the existing `@base-ui/react` Button); `--template vite`; `--yes` skips prompts; `--pointer` enables `cursor-pointer` on buttons (satisfies the a11y `cursor-pointer` rule).
- If the CLI prompts to install the `shadcn` package or asks anything interactive, that step is for **Shaked to run** (project convention: the maintainer runs interactive CLIs) — hand it off, don't guess answers.
- `init` may rewrite `src/index.css` to default tokens and may touch `vite.config.ts`/`tsconfig`. That is expected; Task 2 fully rewrites `index.css`, and Step 3 below reverts any unwanted config churn.

- [ ] **Step 2: Verify components.json was created and looks right**

Run: `cat web/app/components.json`
Expected: a JSON file with `"style"` beginning with `base-` (e.g. `"base-nova"`), `"tsx": true`, `"iconLibrary": "lucide"`, `tailwind.css` = `"src/index.css"`, and aliases including `"ui": "@/components/ui"`, `"utils": "@/lib/utils"`. If `style` is not a `base-*` value, rerun Step 1 with `--force`.

- [ ] **Step 3: Revert unwanted config churn from init**

Run: `cd web && git status` and `git diff web/app/vite.config.ts web/app/tsconfig*.json web/app/src/components/ui/button.tsx`
Expected: `components.json` and new `src/components/ui/*` files are the intended additions. If `init` modified `vite.config.ts`, `tsconfig*.json`, or the existing `button.tsx`, restore them:

```bash
cd web && git checkout -- web/app/vite.config.ts web/app/tsconfig.json web/app/src/components/ui/button.tsx
```

(Leave `package.json`/lockfile dep additions from init in place — they are fine. Leave `index.css` as-is; Task 2 overwrites it.)

- [ ] **Step 4: Add the component set (non-interactive)**

```bash
cd web/app && bun x shadcn add badge card table input label field progress skeleton separator tooltip alert empty spinner kbd --yes
```

Expected: files appear under `web/app/src/components/ui/`. If any single component name is rejected by this shadcn version, note it and continue with the rest (later tasks that need it will call it out).

- [ ] **Step 5: Run the test (typecheck + lint)**

```bash
cd web && bun run typecheck && bun run lint
```

Expected: no errors. (Generated components are self-consistent; if lint flags formatting in generated files, run `bun run format` and re-lint.)

- [ ] **Step 6: Commit**

```bash
cd web && git add web/app/components.json web/app/src/components/ui web/app/package.json bun.lock
git commit -m "feat(web): install shadcn/ui component set on Base UI"
```

---

## Task 2: Design tokens, dark mode, and mono font

Replaces the default neutral tokens with the Cinema Dark cyan palette and switches the app to dark-only.

**Files:**
- Modify: `web/app/src/index.css` (full rewrite of the token blocks)
- Modify: `web/app/index.html` (add `class="dark"` to `<html>`)
- Modify: `web/app/package.json` (add `@fontsource-variable/geist-mono`)

**Interfaces:**
- Produces: CSS variables `--background`, `--card`, `--primary` (cyan), `--primary-foreground` (dark), `--muted-foreground`, `--ring`, `--radius`, plus `--font-mono` mapped to Geist Mono. All consumed by every later task via Tailwind semantic classes (`bg-background`, `text-primary`, `font-mono`, etc.).

- [ ] **Step 1: Add the Geist Mono variable font dependency**

```bash
cd web/app && bun add @fontsource-variable/geist-mono
```

Expected: `@fontsource-variable/geist-mono` (v5.2.8+) added to `dependencies`.

- [ ] **Step 2: Rewrite `web/app/src/index.css`**

Replace the **entire** file with the following (clobber-proof — this is the full desired content, regardless of what Task 1's `init` left behind):

```css
@import 'tailwindcss';
@import "tw-animate-css";
@import "shadcn/tailwind.css";
@import "@fontsource-variable/geist";
@import "@fontsource-variable/geist-mono";

@custom-variant dark (&:is(.dark *));

@theme inline {
    --font-heading: var(--font-sans);
    --font-sans: 'Geist Variable', sans-serif;
    --font-mono: 'Geist Mono Variable', ui-monospace, monospace;
    --color-sidebar-ring: var(--sidebar-ring);
    --color-sidebar-border: var(--sidebar-border);
    --color-sidebar-accent-foreground: var(--sidebar-accent-foreground);
    --color-sidebar-accent: var(--sidebar-accent);
    --color-sidebar-primary-foreground: var(--sidebar-primary-foreground);
    --color-sidebar-primary: var(--sidebar-primary);
    --color-sidebar-foreground: var(--sidebar-foreground);
    --color-sidebar: var(--sidebar);
    --color-chart-5: var(--chart-5);
    --color-chart-4: var(--chart-4);
    --color-chart-3: var(--chart-3);
    --color-chart-2: var(--chart-2);
    --color-chart-1: var(--chart-1);
    --color-ring: var(--ring);
    --color-input: var(--input);
    --color-border: var(--border);
    --color-destructive: var(--destructive);
    --color-accent-foreground: var(--accent-foreground);
    --color-accent: var(--accent);
    --color-muted-foreground: var(--muted-foreground);
    --color-muted: var(--muted);
    --color-secondary-foreground: var(--secondary-foreground);
    --color-secondary: var(--secondary);
    --color-primary-foreground: var(--primary-foreground);
    --color-primary: var(--primary);
    --color-popover-foreground: var(--popover-foreground);
    --color-popover: var(--popover);
    --color-card-foreground: var(--card-foreground);
    --color-card: var(--card);
    --color-foreground: var(--foreground);
    --color-background: var(--background);
    --radius-sm: calc(var(--radius) * 0.6);
    --radius-md: calc(var(--radius) * 0.8);
    --radius-lg: var(--radius);
    --radius-xl: calc(var(--radius) * 1.4);
    --radius-2xl: calc(var(--radius) * 1.8);
    --radius-3xl: calc(var(--radius) * 2.2);
    --radius-4xl: calc(var(--radius) * 2.6);
}

/* Cinema Dark — dark-only v1. Light values are intentionally identical to
   dark for now; a real light theme is a later additive pass. */
:root {
    --radius: 0.5rem;
    --background: oklch(0.13 0.005 285);
    --foreground: oklch(0.93 0.003 285);
    --card: oklch(0.17 0.005 285);
    --card-foreground: oklch(0.93 0.003 285);
    --popover: oklch(0.19 0.005 285);
    --popover-foreground: oklch(0.93 0.003 285);
    --primary: oklch(0.79 0.09 226);
    --primary-foreground: oklch(0.2 0.02 240);
    --secondary: oklch(0.23 0.005 285);
    --secondary-foreground: oklch(0.93 0.003 285);
    --muted: oklch(0.21 0.005 285);
    --muted-foreground: oklch(0.64 0.01 285);
    --accent: oklch(0.23 0.005 285);
    --accent-foreground: oklch(0.84 0.09 226);
    --destructive: oklch(0.62 0.2 20);
    --border: oklch(1 0 0 / 8%);
    --input: oklch(1 0 0 / 14%);
    --ring: oklch(0.79 0.09 226);
    --chart-1: oklch(0.79 0.09 226);
    --chart-2: oklch(0.7 0.1 200);
    --chart-3: oklch(0.62 0.11 260);
    --chart-4: oklch(0.72 0.14 85);
    --chart-5: oklch(0.55 0.1 300);
    --sidebar: oklch(0.15 0.005 285);
    --sidebar-foreground: oklch(0.93 0.003 285);
    --sidebar-primary: oklch(0.79 0.09 226);
    --sidebar-primary-foreground: oklch(0.2 0.02 240);
    --sidebar-accent: oklch(0.23 0.005 285);
    --sidebar-accent-foreground: oklch(0.93 0.003 285);
    --sidebar-border: oklch(1 0 0 / 8%);
    --sidebar-ring: oklch(0.79 0.09 226);
}

.dark {
    --background: oklch(0.13 0.005 285);
    --foreground: oklch(0.93 0.003 285);
    --card: oklch(0.17 0.005 285);
    --card-foreground: oklch(0.93 0.003 285);
    --popover: oklch(0.19 0.005 285);
    --popover-foreground: oklch(0.93 0.003 285);
    --primary: oklch(0.79 0.09 226);
    --primary-foreground: oklch(0.2 0.02 240);
    --secondary: oklch(0.23 0.005 285);
    --secondary-foreground: oklch(0.93 0.003 285);
    --muted: oklch(0.21 0.005 285);
    --muted-foreground: oklch(0.64 0.01 285);
    --accent: oklch(0.23 0.005 285);
    --accent-foreground: oklch(0.84 0.09 226);
    --destructive: oklch(0.62 0.2 20);
    --border: oklch(1 0 0 / 8%);
    --input: oklch(1 0 0 / 14%);
    --ring: oklch(0.79 0.09 226);
    --chart-1: oklch(0.79 0.09 226);
    --chart-2: oklch(0.7 0.1 200);
    --chart-3: oklch(0.62 0.11 260);
    --chart-4: oklch(0.72 0.14 85);
    --chart-5: oklch(0.55 0.1 300);
    --sidebar: oklch(0.15 0.005 285);
    --sidebar-foreground: oklch(0.93 0.003 285);
    --sidebar-primary: oklch(0.79 0.09 226);
    --sidebar-primary-foreground: oklch(0.2 0.02 240);
    --sidebar-accent: oklch(0.23 0.005 285);
    --sidebar-accent-foreground: oklch(0.93 0.003 285);
    --sidebar-border: oklch(1 0 0 / 8%);
    --sidebar-ring: oklch(0.79 0.09 226);
}

@layer base {
  * {
    @apply border-border outline-ring/50;
    }
  body {
    @apply bg-background text-foreground;
    }
  html {
    @apply font-sans;
    }
}
```

- [ ] **Step 3: Force dark mode on `<html>` in `web/app/index.html`**

Change the opening `<html>` tag and `<head>` so the app is always dark and the tab background matches. Replace:

```html
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Luna Web</title>
  </head>
```

with (favicon + full SEO come in Task 3; here only the `class` and `color-scheme`):

```html
<html lang="en" class="dark" style="color-scheme: dark">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Luna Web</title>
  </head>
```

- [ ] **Step 4: Run the test**

```bash
cd web && bun run typecheck && bun run lint
```
Expected: no errors. Then `bun --filter @luna-web/app dev`, open the URL: the landing screen background is now near-black (`#0a0a0c`), text is off-white, and the existing "Pick folder" button fill is **cyan with dark text** (not white-on-dark). No white flash.

- [ ] **Step 5: Commit**

```bash
cd web && git add web/app/src/index.css web/app/index.html web/app/package.json bun.lock
git commit -m "feat(web): Cinema Dark tokens + dark-only + Geist Mono"
```

---

## Task 3: Brand assets, favicon, and SEO/OG metadata

Brings the real Luna logo into the app and states the privacy/product story through page metadata (not UI slogans).

**Files:**
- Create: `web/app/public/luna-logo.webp`, `web/app/public/luna-logo-lg.webp`, `web/app/public/favicon.ico`, `web/app/public/og.png`
- Delete: `web/app/public/favicon.svg` (abstract placeholder — not the brand)
- Modify: `web/app/index.html` (favicon + meta)

- [ ] **Step 1: Copy the logo masters into the app's public dir**

```bash
cd "E:/Coding/LunaApp"
cp Assets/luna-logo.webp     web/app/public/luna-logo.webp
cp Assets/luna-logo-lg.webp  web/app/public/luna-logo-lg.webp
cp Assets/luna-logo.ico      web/app/public/favicon.ico
rm web/app/public/favicon.svg
```

Expected: three brand files present in `web/app/public/`, the placeholder `favicon.svg` gone.

- [ ] **Step 2: Generate the social-preview image (`og.png`, 1200×630)**

The OG image is the logo centered on the app background. This is a one-time static asset — generate it with a disposable local Python/Pillow script (Pillow is a local tool, **not** a project dependency). Write `web/tools/make-og.py`:

```python
# Disposable: composes web/app/public/og.png (1200x630) = Luna logo centered
# on the app background color. Run once: python web/tools/make-og.py
from PIL import Image

W, H = 1200, 630
BG = (10, 10, 12, 255)  # #0a0a0c, matches --background

canvas = Image.new("RGBA", (W, H), BG)
logo = Image.open("Assets/luna-logo-lg.webp").convert("RGBA")
# scale logo to ~46% of height
target_h = int(H * 0.46)
scale = target_h / logo.height
logo = logo.resize((int(logo.width * scale), target_h), Image.LANCZOS)
canvas.alpha_composite(logo, ((W - logo.width) // 2, (H - logo.height) // 2))
canvas.convert("RGB").save("web/app/public/og.png", "PNG")
print("wrote web/app/public/og.png")
```

Run (from repo root; installs Pillow into the local interpreter only if missing):

```bash
cd "E:/Coding/LunaApp" && python -m pip install --quiet Pillow && python web/tools/make-og.py
```

Expected: `web/app/public/og.png` created (1200×630). Verify by opening it — a centered cat+moon logo on near-black. Keep `web/tools/make-og.py` in the repo (small, documents how the asset was made) or delete it; either is fine — it is not wired into the build.

- [ ] **Step 3: Write the full `<head>` metadata in `web/app/index.html`**

Replace the entire `<head>` block with (uses absolute URLs on the production host `luna.ozer2.one` for OG, per spec §18):

```html
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <link rel="icon" href="/favicon.ico" sizes="any" />
    <meta name="theme-color" content="#0a0a0c" />

    <title>Luna — camera reports in your browser</title>
    <meta
      name="description"
      content="Luna generates camera reports from a footage folder entirely in your browser. Scanning, thumbnail extraction, and PDF/CSV export run on your device — no upload, no install."
    />

    <meta property="og:type" content="website" />
    <meta property="og:site_name" content="Luna" />
    <meta property="og:title" content="Luna — camera reports in your browser" />
    <meta
      property="og:description"
      content="Point Luna at a camera card and get per-clip thumbnails, metadata, reel grouping, and a downloadable PDF/CSV report — processed entirely on your device."
    />
    <meta property="og:url" content="https://luna.ozer2.one/" />
    <meta property="og:image" content="https://luna.ozer2.one/og.png" />
    <meta property="og:image:width" content="1200" />
    <meta property="og:image:height" content="630" />

    <meta name="twitter:card" content="summary_large_image" />
    <meta name="twitter:title" content="Luna — camera reports in your browser" />
    <meta
      name="twitter:description"
      content="Per-clip thumbnails, metadata, reel grouping, and PDF/CSV camera reports — processed entirely in your browser."
    />
    <meta name="twitter:image" content="https://luna.ozer2.one/og.png" />
  </head>
```

- [ ] **Step 4: Run the test**

```bash
cd web && bun run typecheck && bun run lint
```
Expected: no errors. Then in the dev server, confirm the **browser tab shows the cat+moon favicon** (not the old abstract mark) and there is no 404 for `/favicon.svg` in the console/network tab.

- [ ] **Step 5: Commit**

```bash
cd web && git add web/app/public web/app/index.html web/tools/make-og.py
git rm --cached web/app/public/favicon.svg 2>/dev/null; true
git commit -m "feat(web): Luna logo assets, favicon, and SEO/OG metadata"
```

---

## Task 4: App shell, Logo component, and version wiring

Adds the slim header and page container, and a reusable `<Logo>` mark. Wires the app version for the header via a Vite define.

**Files:**
- Create: `web/app/src/components/logo.tsx`
- Create: `web/app/src/components/app-shell.tsx`
- Create: `web/app/src/vite-env.d.ts`
- Modify: `web/app/vite.config.ts`
- Modify: `web/app/src/routes/__root.tsx`
- Modify: `web/app/src/features/scan/scan-screen.tsx` (remove now-duplicated outer centering/heading — the shell owns chrome)

**Interfaces:**
- Produces: `Logo` (`{ className?: string }` → `<img>`), `AppShell` (`{ children: ReactNode }`), and the global `__APP_VERSION__: string`.
- Consumes: `buttonVariants` from `@/components/ui/button`, `cn` from `@/lib/utils`.

- [ ] **Step 1: Inject the app version in `web/app/vite.config.ts`**

Replace the file with:

```ts
import { readFileSync } from 'node:fs'
import { fileURLToPath, URL } from 'node:url'
import tailwindcss from '@tailwindcss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const pkg = JSON.parse(
  readFileSync(new URL('./package.json', import.meta.url), 'utf8'),
) as { version: string }

export default defineConfig({
  plugins: [tanstackRouter({ target: 'react', autoCodeSplitting: true }), react(), tailwindcss()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  define: { __APP_VERSION__: JSON.stringify(pkg.version) },
})
```

> Reading via `fs` (not a JSON `import`) avoids depending on `resolveJsonModule` in the node tsconfig, so `bun run typecheck` stays green.

- [ ] **Step 2: Declare the global in `web/app/src/vite-env.d.ts`**

```ts
/// <reference types="vite/client" />

declare const __APP_VERSION__: string
```

- [ ] **Step 3: Create `web/app/src/components/logo.tsx`**

```tsx
export function Logo({ className }: { className?: string }) {
  return (
    <img
      src="/luna-logo.webp"
      alt="Luna"
      width={317}
      height={333}
      loading="eager"
      decoding="async"
      className={className}
    />
  )
}
```

- [ ] **Step 4: Create `web/app/src/components/app-shell.tsx`**

Use a plain styled anchor for the Docs link (a plain `<a>` is required anyway — it must be a real navigation to the edge-routed `/docs`, per spec §8.13 — so this side-steps any `render`-prop uncertainty in the generated Button):

```tsx
import type { ReactNode } from 'react'
import { Logo } from '@/components/logo'
import { buttonVariants } from '@/components/ui/button'
import { cn } from '@/lib/utils'

export function AppShell({ children }: { children: ReactNode }) {
  return (
    <div className="flex min-h-dvh flex-col">
      <header className="bg-background/80 sticky top-0 z-40 border-b backdrop-blur">
        <div className="mx-auto flex h-14 max-w-5xl items-center justify-between px-6">
          <a href="/" className="flex items-center gap-2">
            <Logo className="h-7 w-auto" />
            <span className="text-base font-semibold tracking-tight">Luna</span>
          </a>
          <nav className="text-muted-foreground flex items-center gap-2 text-sm">
            <a href="/docs" className={cn(buttonVariants({ variant: 'ghost', size: 'sm' }))}>
              Docs
            </a>
            <span className="tabular-nums">v{__APP_VERSION__}</span>
          </nav>
        </div>
      </header>
      <main className="mx-auto w-full max-w-5xl flex-1 px-6 py-10">{children}</main>
    </div>
  )
}
```

(`buttonVariants` is exported by the existing `web/app/src/components/ui/button.tsx` — confirmed present.)

- [ ] **Step 5: Wrap routed content with the shell in `web/app/src/routes/__root.tsx`**

```tsx
import { createRootRoute, Outlet } from '@tanstack/react-router'
import { AppShell } from '@/components/app-shell'
import { CapabilityGate } from '@/components/capability-gate'

export const Route = createRootRoute({
  component: () => (
    <CapabilityGate>
      <AppShell>
        <Outlet />
      </AppShell>
    </CapabilityGate>
  ),
})
```

- [ ] **Step 6: Strip the duplicated outer chrome from `web/app/src/features/scan/scan-screen.tsx`**

The shell now provides the page container and the `Luna` wordmark lives in the header, so the screen's `<main>` wrapper and the big `<h1>Luna Web</h1>` are redundant. Change the outer wrapper and drop the `<h1>`:

Replace:
```tsx
  return (
    <main className="mx-auto flex min-h-screen max-w-4xl flex-col items-center justify-center gap-6 px-6 py-12">
      <h1 className="font-heading text-3xl font-semibold">Luna Web</h1>
```
with:
```tsx
  return (
    <div className="flex flex-col gap-6">
```
and change the matching closing `</main>` at the end of the component to `</div>`.

(The idle-state hero heading is re-added properly in Task 7; the other phases render into this container.)

- [ ] **Step 7: Run the test**

```bash
cd web && bun run typecheck && bun run lint
```
Expected: no errors. Then in the dev server: a sticky header shows the cat+moon logo + "Luna" on the left and "Docs  v0.0.0" on the right, hairline bottom border, blurred translucent background when content scrolls under it. The old giant "Luna Web" H1 is gone.

- [ ] **Step 8: Commit**

```bash
cd web && git add web/app/vite.config.ts web/app/src/vite-env.d.ts web/app/src/components/logo.tsx web/app/src/components/app-shell.tsx web/app/src/routes/__root.tsx web/app/src/features/scan/scan-screen.tsx
git commit -m "feat(web): app shell with Luna logo header + version"
```

---

## Task 5: Capability gate restyle

Brings the gate onto tokens + the logo (currently hardcoded zinc).

**Files:**
- Modify: `web/app/src/components/capability-gate.tsx`

**Interfaces:**
- Consumes: `Logo` from `@/components/logo`.

- [ ] **Step 1: Rewrite the unsupported-branch markup**

Replace the returned `<main>` block (the unsupported branch) with:

```tsx
  return (
    <main className="bg-background text-foreground flex min-h-dvh flex-col items-center justify-center gap-5 px-6 text-center">
      <Logo className="h-12 w-auto" />
      <h1 className="text-2xl font-semibold">Luna needs a Chromium browser</h1>
      <p className="text-muted-foreground max-w-md text-sm leading-relaxed">
        Luna reads your footage folder directly on your device using the File System Access API,
        available in Chrome, Edge, and other Chromium browsers. Please open this page in one of
        those to continue. Nothing is ever uploaded.
      </p>
    </main>
  )
```

Add the import at the top: `import { Logo } from '@/components/logo'`.

- [ ] **Step 2: Run the test**

```bash
cd web && bun run typecheck && bun run lint
```
Expected: no errors. Manual check (optional): temporarily force the unsupported branch by editing `isBrowserSupported` to return `false`, confirm the gate shows the logo on the dark background with tokenized text, then revert.

- [ ] **Step 3: Commit**

```bash
cd web && git add web/app/src/components/capability-gate.tsx
git commit -m "feat(web): restyle capability gate with logo + tokens"
```

---

## Task 6: Shared clip presentation (clip-row, thumbs, badges, stat tile)

Restyles the pieces reused by both the processing and results screens, plus a shared stat tile used by summary + results.

**Files:**
- Create: `web/app/src/components/stat-tile.tsx`
- Modify: `web/app/src/features/scan/clip-row.tsx`

**Interfaces:**
- Produces: `StatTile` (`{ label: string; value: string }`).
- Consumes: `Badge`, `Skeleton` from `@/components/ui/*`; existing store selectors in `clip-row.tsx` are unchanged.

- [ ] **Step 1: Create `web/app/src/components/stat-tile.tsx`**

```tsx
export function StatTile({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-1">
      <span className="text-muted-foreground text-xs font-medium tracking-wide uppercase">
        {label}
      </span>
      <span className="font-mono text-2xl tabular-nums">{value}</span>
    </div>
  )
}
```

- [ ] **Step 2: Restyle thumbnails + status in `web/app/src/features/scan/clip-row.tsx`**

Change the skeleton placeholders to the `Skeleton` component and the status text to `Badge`. Update the imports:

```tsx
import { Badge } from '@/components/ui/badge'
import { Skeleton } from '@/components/ui/skeleton'
```

Replace the queued/decoding placeholder block:
```tsx
        <div className="flex gap-1">
          {[0, 1, 2].map((i) => (
            <div key={i} className="bg-muted h-14 w-24 animate-pulse rounded" />
          ))}
        </div>
```
with:
```tsx
        <div className="flex gap-1">
          {[0, 1, 2].map((i) => (
            <Skeleton key={i} className="h-14 w-24 rounded-md" />
          ))}
        </div>
```

Give the metadata columns a mono treatment — change the filename + metadata `<span>`s so numeric/technical cells use `font-mono`. Replace the grid block:
```tsx
      <div className="grid grid-cols-[1fr_auto_auto_auto_auto_auto] items-center gap-x-4">
        <span className="truncate">{clip.relativePath}</span>
        <span className="text-muted-foreground tabular-nums">
          {m?.width !== undefined && m?.height !== undefined ? `${m.width}×${m.height}` : '—'}
        </span>
        <span className="text-muted-foreground">{m?.codec ?? '—'}</span>
        <span className="text-muted-foreground tabular-nums">
          {m?.frameRate !== undefined ? `${m.frameRate} fps` : '—'}
        </span>
        <span className="text-muted-foreground tabular-nums">
          {m?.durationSeconds !== undefined ? formatDuration(m.durationSeconds) : '—'}
        </span>
        <span className="text-muted-foreground tabular-nums">
          {status === 'done' ? (
            formatBytes(clip.sizeBytes)
          ) : (
            <StatusBadge status={status} error={error} />
          )}
        </span>
      </div>
```
with:
```tsx
      <div className="grid grid-cols-[1fr_auto_auto_auto_auto_auto] items-center gap-x-4">
        <span className="truncate font-mono text-[0.8rem]">{clip.relativePath}</span>
        <span className="text-muted-foreground font-mono tabular-nums">
          {m?.width !== undefined && m?.height !== undefined ? `${m.width}×${m.height}` : '—'}
        </span>
        <span className="text-muted-foreground font-mono">{m?.codec ?? '—'}</span>
        <span className="text-muted-foreground font-mono tabular-nums">
          {m?.frameRate !== undefined ? `${m.frameRate} fps` : '—'}
        </span>
        <span className="text-muted-foreground font-mono tabular-nums">
          {m?.durationSeconds !== undefined ? formatDuration(m.durationSeconds) : '—'}
        </span>
        <span className="font-mono tabular-nums">
          {status === 'done' ? (
            formatBytes(clip.sizeBytes)
          ) : (
            <StatusBadge status={status} error={error} />
          )}
        </span>
      </div>
```

Add a subtle hover lift to successful thumbnails — in `ThumbStrip`, change the `<img>` className:
```tsx
            className="h-14 rounded object-cover"
```
to:
```tsx
            className="border-border h-14 rounded border object-cover transition-transform hover:scale-[1.02]"
```

Replace `StatusBadge` at the bottom of the file with a `Badge`-based version:
```tsx
export function StatusBadge({ status, error }: { status: string; error?: string }) {
  if (status === 'failed') {
    return (
      <Badge variant="destructive" title={error}>
        failed
      </Badge>
    )
  }
  return (
    <Badge variant="secondary" className="text-muted-foreground">
      {status}…
    </Badge>
  )
}
```

Restyle the RAW placeholder tile and `RawSection` list to use tokens (they already do). In `RawSection`, wrap the list in a `Card`-like border (already `rounded-lg border`) — leave as-is; no change required beyond what exists.

- [ ] **Step 3: Run the test**

```bash
cd web && bun run typecheck && bun run lint
```
Expected: no errors. (Visual verification of these pieces happens in Tasks 9–10 where they render in context.)

- [ ] **Step 4: Commit**

```bash
cd web && git add web/app/src/components/stat-tile.tsx web/app/src/features/scan/clip-row.tsx
git commit -m "feat(web): restyle clip rows, thumbnails, badges + stat tile"
```

---

## Task 7: Landing (idle) + recent list

The hero screen with the logo, the single glowing CTA, and the recent-sources list as cards (and the `✕`→lucide `X` fix).

**Files:**
- Modify: `web/app/src/features/scan/scan-screen.tsx` (idle branch)
- Modify: `web/app/src/features/scan/recent-list.tsx`

**Interfaces:**
- Consumes: `Logo`, `Button`, lucide `X`, `FolderOpen`.

- [ ] **Step 1: Restyle the idle branch in `scan-screen.tsx`**

Add imports at the top:
```tsx
import { Logo } from '@/components/logo'
```

Replace the idle block:
```tsx
      {phase === 'idle' && (
        <>
          <p className="text-muted-foreground">
            Pick a footage folder — everything stays on this device.
          </p>
          <Button onClick={() => void pickAndScan()}>Pick folder</Button>
          <RecentList />
        </>
      )}
```
with a centered hero (the CTA carries the one accent glow via an inline `boxShadow`, per the "single CTA per screen" rule):
```tsx
      {phase === 'idle' && (
        <div className="flex flex-col items-center gap-6 py-16 text-center">
          <Logo className="h-16 w-auto" />
          <div className="space-y-2">
            <h1 className="text-2xl font-semibold tracking-tight">Camera reports, in your browser</h1>
            <p className="text-muted-foreground max-w-md text-sm leading-relaxed">
              Pick a footage folder to get per-clip thumbnails, metadata, and a PDF/CSV report.
              Everything stays on this device.
            </p>
          </div>
          <Button
            size="lg"
            onClick={() => void pickAndScan()}
            style={{ boxShadow: '0 0 24px oklch(0.72 0.14 245 / 0.25)' }}
          >
            Pick folder
          </Button>
          <p className="text-muted-foreground font-mono text-xs">
            MOV · MP4 · MXF · MKV · AVI · MTS · and more
          </p>
          <RecentList />
        </div>
      )}
```

- [ ] **Step 2: Rewrite `web/app/src/features/scan/recent-list.tsx`**

Replace the whole file — cards with a folder icon, hover lift, and a lucide `X` remove button (no `✕` glyph):

```tsx
import { FolderOpen, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  forgetSource,
  listRecentSources,
  type StoredRecentSource,
} from '@/persistence/recent-sources'
import { scanFrom } from './run-scan'

type Entry = { key: number } & StoredRecentSource

export function RecentList() {
  const [entries, setEntries] = useState<Entry[]>([])

  useEffect(() => {
    void listRecentSources().then(setEntries)
  }, [])

  if (entries.length === 0) return null

  return (
    <section className="w-full max-w-md text-left">
      <h2 className="text-muted-foreground mb-2 text-xs font-medium tracking-wide uppercase">
        Recent folders
      </h2>
      <ul className="flex flex-col gap-1.5">
        {entries.map((e) => (
          <li key={e.key}>
            <div className="bg-card hover:border-input flex items-center justify-between rounded-lg border px-3 py-2 transition-colors">
              <button
                type="button"
                className="flex min-w-0 items-center gap-2 text-left text-sm"
                onClick={() => void scanFrom(e.handle)}
              >
                <FolderOpen className="text-muted-foreground size-4 shrink-0" />
                <span className="truncate">{e.name}</span>
              </button>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={`Remove ${e.name} from recent folders`}
                onClick={() => {
                  void forgetSource(e.key).then(() => listRecentSources().then(setEntries))
                }}
              >
                <X />
              </Button>
            </div>
          </li>
        ))}
      </ul>
    </section>
  )
}
```

- [ ] **Step 3: Run the test**

```bash
cd web && bun run typecheck && bun run lint
```
Expected: no errors. In the dev server on the landing screen: centered logo, heading, muted description, one cyan **Pick folder** button with a soft cyan glow, a mono formats hint, and (if you have prior sessions) recent folders as cards with a folder icon and an `X` icon button. No `✕` text glyph anywhere.

- [ ] **Step 4: Commit**

```bash
cd web && git add web/app/src/features/scan/scan-screen.tsx web/app/src/features/scan/recent-list.tsx
git commit -m "feat(web): landing hero + recent-folder cards"
```

---

## Task 8: Scanning + pre-scan summary

**Files:**
- Modify: `web/app/src/features/scan/scan-screen.tsx` (scanning + summary branches)

**Interfaces:**
- Consumes: `Spinner`, `Card`/`CardHeader`/`CardTitle`/`CardContent`, `Alert`/`AlertTitle`/`AlertDescription`, `Button`, `StatTile`, lucide `TriangleAlert`.

- [ ] **Step 1: Add imports to `scan-screen.tsx`**

```tsx
import { TriangleAlert } from 'lucide-react'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Spinner } from '@/components/ui/spinner'
import { StatTile } from '@/components/stat-tile'
```

- [ ] **Step 2: Restyle the scanning branch**

Replace:
```tsx
      {phase === 'scanning' && (
        <p className="text-muted-foreground" aria-live="polite">
          Scanning {sourceName}…{' '}
          {progress ? `${progress.filesSeen} files, ${progress.clipsFound} clips` : ''}
        </p>
      )}
```
with:
```tsx
      {phase === 'scanning' && (
        <div className="flex items-center gap-3 py-16" aria-live="polite">
          <Spinner className="size-5" />
          <p className="text-muted-foreground">
            Scanning {sourceName}…{' '}
            <span className="font-mono tabular-nums">
              {progress ? `${progress.filesSeen} files, ${progress.clipsFound} clips` : ''}
            </span>
          </p>
        </div>
      )}
```

- [ ] **Step 3: Restyle the summary branch**

Replace the entire `{phase === 'summary' && summary && ( ... )}` block with:
```tsx
      {phase === 'summary' && summary && (
        <Card className="w-full">
          <CardHeader>
            <CardTitle className="truncate">{sourceName}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-6">
            <dl className="grid grid-cols-3 gap-4">
              <StatTile label="Clips" value={String(summary.clipCount)} />
              <StatTile label="Total size" value={formatBytes(summary.totalClipSizeBytes)} />
              <StatTile label="RAW (unsupported)" value={String(summary.rawCount)} />
            </dl>
            {summary.rawCount > 0 && (
              <Alert>
                <TriangleAlert />
                <AlertTitle>{summary.rawCount} RAW file(s) can't be decoded in a browser</AlertTitle>
                <AlertDescription>
                  ARRIRAW / R3D / BRAW files were detected. They'll be listed in the report without
                  thumbnails.
                </AlertDescription>
              </Alert>
            )}
            <div className="flex gap-3">
              <Button onClick={() => void startProcessing()}>
                Process {summary.clipCount} clips
              </Button>
              <Button variant="outline" onClick={resetScan}>
                Cancel
              </Button>
            </div>
          </CardContent>
        </Card>
      )}
```

> The RAW `Alert` uses the default variant tinted amber. If a distinct amber is wanted beyond the default border, add `className="border-amber-500/30 [&>svg]:text-amber-400"` to the `Alert`. Keep it icon + text (never color alone).

- [ ] **Step 4: Run the test**

```bash
cd web && bun run typecheck && bun run lint
```
Expected: no errors. To view: pick a folder that contains media (and ideally a `.braw`/`.r3d` to see the amber alert). Confirm three stat tiles with mono numerals, the amber alert with a warning icon when RAW is present, and Process (cyan) / Cancel (outline) buttons.

- [ ] **Step 5: Commit**

```bash
cd web && git add web/app/src/features/scan/scan-screen.tsx
git commit -m "feat(web): restyle scanning + pre-scan summary"
```

---

## Task 9: Processing screen

**Files:**
- Modify: `web/app/src/features/scan/scan-screen.tsx` (processing/thumbnailing branch)

**Interfaces:**
- Consumes: `Progress`, `Card`, `Button`, existing `ClipRow`/`RawSection`.

- [ ] **Step 1: Add the Progress import**

```tsx
import { Progress } from '@/components/ui/progress'
```

- [ ] **Step 2: Restyle the processing branch**

Replace the entire `{(phase === 'processing' || phase === 'thumbnailing') && ( ... )}` block with (adds a determinate progress bar computed from the counters already in scope):
```tsx
      {(phase === 'processing' || phase === 'thumbnailing') && (
        <section className="w-full space-y-4">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <h2 className="text-lg font-medium" aria-live="polite">
                {phase === 'processing'
                  ? `Reading metadata… ${processedCount}/${clips.length}`
                  : `Generating thumbnails… ${thumbDoneCount}/${thumbTotal}`}
              </h2>
              <Button variant="outline" size="sm" onClick={resetScan}>
                Start over
              </Button>
            </div>
            <Progress
              value={
                phase === 'processing'
                  ? clips.length > 0
                    ? (processedCount / clips.length) * 100
                    : 0
                  : thumbTotal > 0
                    ? (thumbDoneCount / thumbTotal) * 100
                    : 0
              }
            />
          </div>
          <Card className="overflow-hidden py-0">
            <ul className="divide-y">
              {clips.map((c) => (
                <ClipRow key={c.id} clipId={c.id} />
              ))}
            </ul>
          </Card>
          <RawSection raw={raw} />
        </section>
      )}
```

- [ ] **Step 3: Run the test**

```bash
cd web && bun run typecheck && bun run lint
```
Expected: no errors. In the dev server, process a folder: the header shows a live `n/m` count, a cyan determinate progress bar advances, clip rows show shimmer skeleton thumbnails that resolve to images, and status badges update. RAW files appear in the RAW section below.

- [ ] **Step 4: Commit**

```bash
cd web && git add web/app/src/features/scan/scan-screen.tsx
git commit -m "feat(web): restyle processing screen with progress + skeletons"
```

---

## Task 10: Results workspace, cover form, export toolbar

**Files:**
- Modify: `web/app/src/features/report/report-workspace.tsx`
- Modify: `web/app/src/features/report/cover-form.tsx`
- Modify: `web/app/src/features/export/export-buttons.tsx`

**Interfaces:**
- Consumes: `Card`, `Separator`, `StatTile`, `Button`, `Input`, `Label` (or `Field`).

- [ ] **Step 1: Restyle `report-workspace.tsx`**

Update imports (remove the local `Stat` helper, use `StatTile`):
```tsx
import { StatTile } from '@/components/stat-tile'
import { Card, CardContent } from '@/components/ui/card'
```

Replace the stats `<dl>` + its local `Stat` function. Change the `<dl>`:
```tsx
      <dl className="mb-6 grid grid-cols-5 gap-4 text-center">
        <Stat label="Cards" value={String(model.stats.cardCount)} />
        <Stat label="Clips" value={String(model.stats.clipCount)} />
        <Stat label="Duration" value={formatDuration(model.stats.totalDurationSeconds)} />
        <Stat label="Size" value={formatBytes(model.stats.totalSizeBytes)} />
        <Stat label="RAW" value={String(model.stats.rawCount)} />
      </dl>
```
to:
```tsx
      <Card className="mb-6">
        <CardContent>
          <dl className="grid grid-cols-5 gap-4">
            <StatTile label="Cards" value={String(model.stats.cardCount)} />
            <StatTile label="Clips" value={String(model.stats.clipCount)} />
            <StatTile label="Duration" value={formatDuration(model.stats.totalDurationSeconds)} />
            <StatTile label="Size" value={formatBytes(model.stats.totalSizeBytes)} />
            <StatTile label="RAW" value={String(model.stats.rawCount)} />
          </dl>
        </CardContent>
      </Card>
```
and delete the local `Stat` function at the bottom of the file.

Wrap each reel's clip list in a `Card` and make the reel heading sticky. Replace:
```tsx
      {model.reels.map((reel) => (
        <section key={reel.name} className="mb-6">
          <h3 className="mb-2 flex items-baseline gap-3 text-lg font-medium">
            {reel.name}
            <span className="text-muted-foreground text-sm">
              {reel.clips.length} clips · {formatBytes(reel.stats.totalSizeBytes)}
            </span>
          </h3>
          <ul className="divide-y rounded-lg border">
            {reel.clips.map((clip) => (
              <ClipRow key={clip.id} clipId={clip.id} />
            ))}
          </ul>
        </section>
      ))}
```
with:
```tsx
      {model.reels.map((reel) => (
        <section key={reel.name} className="mb-6">
          <h3 className="bg-background/80 sticky top-14 z-10 mb-2 flex items-baseline gap-3 py-2 text-lg font-medium backdrop-blur">
            {reel.name}
            <span className="text-muted-foreground font-mono text-sm tabular-nums">
              {reel.clips.length} clips · {formatBytes(reel.stats.totalSizeBytes)}
            </span>
          </h3>
          <Card className="overflow-hidden py-0">
            <ul className="divide-y">
              {reel.clips.map((clip) => (
                <ClipRow key={clip.id} clipId={clip.id} />
              ))}
            </ul>
          </Card>
        </section>
      ))}
```

Remove the now-inaccurate "PDF export arrives in the next milestone." line if PDF export is wired (it is, per `export-buttons.tsx` importing `./pdf-exporter`). Replace:
```tsx
      <RawSection raw={model.raw} />
      <p className="text-muted-foreground mt-3 text-sm">
        PDF export arrives in the next milestone.
      </p>
```
with:
```tsx
      <RawSection raw={model.raw} />
```

- [ ] **Step 2: Restyle the cover form onto Input/Label in `cover-form.tsx`**

Replace the `<section>`/`<input>` markup so it uses `Card` + `Input` + `Label` (keep all TanStack Form wiring identical). Update imports:
```tsx
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
```

Replace the returned `<section>...</section>` of `CoverForm` with:
```tsx
    <Card className="w-full">
      <CardHeader>
        <CardTitle>Report details</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-2 gap-4">
          {TEXT_FIELDS.map(([name, label]) => (
            <form.Field key={name} name={name}>
              {(field) => (
                <div className="flex flex-col gap-1.5">
                  <Label htmlFor={field.name} className="text-muted-foreground">
                    {label}
                  </Label>
                  <Input
                    id={field.name}
                    name={field.name}
                    value={field.state.value}
                    onChange={(e) => field.handleChange(e.target.value)}
                    onBlur={() => {
                      field.handleBlur()
                      setCoverFields({ [field.name]: field.state.value })
                    }}
                  />
                </div>
              )}
            </form.Field>
          ))}
          <LogoPicker />
        </div>
      </CardContent>
    </Card>
```

Update `LogoPicker`'s label markup to match (use `Label` + keep the native file input, styled):
```tsx
  return (
    <div className="flex flex-col gap-1.5">
      <Label htmlFor="cover-logo" className="text-muted-foreground">
        Logo
      </Label>
      <input
        id="cover-logo"
        type="file"
        accept="image/*"
        className="text-muted-foreground file:bg-secondary file:text-secondary-foreground hover:file:bg-secondary/80 text-sm file:mr-3 file:rounded-md file:border-0 file:px-3 file:py-1.5 file:text-sm"
        onChange={(e) => {
          const file = e.target.files?.[0]
          if (file) setCoverFields({ logo: file })
        }}
      />
      {previewUrl && (
        <img src={previewUrl} alt="Report logo preview" className="mt-1 h-10 w-auto object-contain" />
      )}
    </div>
  )
```

- [ ] **Step 3: Restyle the export toolbar in `export-buttons.tsx`**

Make PDF the primary (accent) action and CSV outline, with a spinner while busy. Update imports:
```tsx
import { Spinner } from '@/components/ui/spinner'
```
Replace the `<Button ...>` inside the map:
```tsx
        <Button
          key={exporter.id}
          disabled={busy !== null}
          onClick={() => {
            setError(null)
            setBusy(exporter.id)
            runExport(exporter, report)
              .catch((err) => setError(err instanceof Error ? err.message : String(err)))
              .finally(() => setBusy(null))
          }}
        >
          {busy === exporter.id ? 'Exporting…' : `Export ${exporter.label}`}
        </Button>
```
with:
```tsx
        <Button
          key={exporter.id}
          variant={exporter.id === 'pdf' ? 'default' : 'outline'}
          disabled={busy !== null}
          onClick={() => {
            setError(null)
            setBusy(exporter.id)
            runExport(exporter, report)
              .catch((err) => setError(err instanceof Error ? err.message : String(err)))
              .finally(() => setBusy(null))
          }}
        >
          {busy === exporter.id ? (
            <>
              <Spinner /> Exporting…
            </>
          ) : (
            `Export ${exporter.label}`
          )}
        </Button>
```

> If the exporter ids are not literally `'pdf'`/`'csv'`, check `csv-exporter.ts`/`pdf-exporter.ts` for the actual `id` values and match them. Keep exactly one `default` (accent) button.

- [ ] **Step 4: Run the test**

```bash
cd web && bun run typecheck && bun run lint
```
Expected: no errors. In the dev server, after processing completes: a stat card with 5 mono figures, reel sections whose headings stick under the app header while scrolling, a Report-details card with tokenized inputs and a styled file picker, and an export toolbar with a cyan **Export PDF** + outline **Export CSV** that show a spinner while generating. Generate a PDF and CSV to confirm they still download.

- [ ] **Step 5: Commit**

```bash
cd web && git add web/app/src/features/report/report-workspace.tsx web/app/src/features/report/cover-form.tsx web/app/src/features/export/export-buttons.tsx
git commit -m "feat(web): restyle results workspace, cover form, export toolbar"
```

---

## Task 11: Motion, reduced-motion, and final verification

Adds phase-entrance motion, guarantees reduced-motion support, and runs the whole-app visual pass.

**Files:**
- Modify: `web/app/src/features/scan/scan-screen.tsx` (entrance animation on the phase container)
- Modify: `web/app/src/index.css` (reduced-motion safety net)

- [ ] **Step 1: Add a reduced-motion safety net to `index.css`**

Append to the end of `web/app/src/index.css`:
```css
@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 0.001ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.001ms !important;
  }
}
```

- [ ] **Step 2: Add a subtle entrance animation to phase content**

In `scan-screen.tsx`, give the phase container a fade/slide-in using the `tw-animate-css` utilities already imported. Change the outer wrapper introduced in Task 4 Step 6:
```tsx
    <div className="flex flex-col gap-6">
```
to:
```tsx
    <div className="flex animate-in fade-in slide-in-from-bottom-2 flex-col gap-6 duration-300">
```

- [ ] **Step 3: Run the test**

```bash
cd web && bun run typecheck && bun run lint
```
Expected: no errors.

- [ ] **Step 4: Full manual verification pass**

Start the dev server (`cd web && bun --filter @luna-web/app dev`) in Chrome at 1280 and 1440 wide, and walk every phase:

- [ ] Header: logo + wordmark left, Docs + version right, hairline border, blur-on-scroll.
- [ ] Landing: logo hero, single cyan **Pick folder** with soft glow, mono formats hint, recent cards (folder icon + `X` icon-button — **no `✕` glyph**).
- [ ] Scanning: spinner + live mono counts.
- [ ] Summary: 3 stat tiles, amber RAW alert (icon + text) when RAW present, Process (cyan) / Cancel (outline).
- [ ] Processing: determinate cyan progress bar, skeleton→image thumbnails, status badges.
- [ ] Results: 5-figure stat card, sticky reel headers, tokenized cover form + file picker, **Export PDF** (cyan) + **Export CSV** (outline) with spinners; PDF + CSV actually download.
- [ ] Error: trigger a scan error (e.g. cancel the picker) → tokenized destructive message + Back.
- [ ] Focus: Tab through the landing + a form — visible cyan focus rings on every control.
- [ ] Reduced motion: enable "Emulate prefers-reduced-motion: reduce" in Chrome DevTools Rendering → transitions are instant, no shimmer churn.
- [ ] Contrast spot-check (DevTools color picker) on muted text, badges, and the amber alert against their surfaces — all ≥4.5:1.

Fix any issue found, re-run typecheck + lint, and note it in the commit.

- [ ] **Step 5: Commit**

```bash
cd web && git add web/app/src/features/scan/scan-screen.tsx web/app/src/index.css
git commit -m "feat(web): phase entrance motion + reduced-motion safety net"
```

- [ ] **Step 6: Finish the branch**

Use superpowers:finishing-a-development-branch to decide how to integrate (this work is on `feature/luna-web`).

---

## Self-review notes (for the implementer)

- **Spec coverage:** tokens/§2 → Task 2; shadcn/§3 → Task 1 + used throughout; shell/§4 → Task 4; brand assets/§4b → Task 3 (+ Logo in Tasks 4/5/7); screens/§5.1–5.6 → Tasks 5,7,8,9,10; motion & a11y/§6 → Task 11 (+ per-task focus/tokens); out-of-scope/§7 respected (no core/PDF/docs/mobile/light-mode work). SEO metadata replacing in-UI privacy slogans → Task 3.
- **Emoji-icon rule:** the one existing violation (`✕` in `recent-list.tsx`) is fixed in Task 7.
- **Interactive-CLI convention:** Task 1's `init`/`add` are non-interactive (`--yes`); if any prompt appears, hand it to Shaked (project convention) rather than guessing.
- **Assumption to verify at runtime, not blockers:** exact shadcn export names from the generated `components/ui/*` (Task 1 Interfaces flags this) and exact exporter `id` strings (Task 10 Step 3 note). The Docs link uses a plain styled anchor, so no Button `render`-prop dependency. Confirm names against generated code when you reach each task.
