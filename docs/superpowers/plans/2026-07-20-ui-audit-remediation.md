# UI Audit Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the defects and consistency debt from the 2026-07-20 UI audit and ship the approved next-level upgrades: report table view, virtualized clip lists, sticky reel nav with scrollspy, report masthead, and a processing view that collapses completed clips.

**Architecture:** All work is inside `apps/web` (React 19 + Tailwind v4 `@theme` tokens + shadcn "base-nova" style on Base UI + TanStack Router/Store). New primitives come from the shadcn CLI (never hand-rolled); new list virtualization uses `@tanstack/react-virtual`'s `useWindowVirtualizer`. The PDF pipeline is out of scope and must not be touched.

**Tech Stack:** bun, Vite 8, React 19, Tailwind v4, shadcn (base-nova / Base UI), TanStack Router/Store/Form/Virtual, Lucide.

## Global Constraints

- Package manager is **bun only**. Workspace script form: `bun --filter=@luna-web/app run <script>` (NOT `--filter '<pkg>' run <script>` — that form breaks in bun 1.3.14).
- New deps: latest only, via `bun add` in `apps/web`. Only approved new dep: `@tanstack/react-virtual`.
- Missing UI primitives are installed with the shadcn CLI from `apps/web`: `bunx shadcn@latest add <name>`. Never hand-roll a component the registry provides. The project style is `base-nova` (Base UI, not Radix) — after each install, **read the generated file** and match its actual prop API (Base UI uses `render={...}` composition, not Radix `asChild`).
- **No toasts, ever.** Async outcomes render inside the component that triggered them.
- **No emoji/ASCII glyphs as icons** — Lucide only.
- `useSelector` from `@tanstack/react-store` (never the deprecated `useStore`).
- Dark-only theme. `index.css` notes tokens are mirrored in `apps/docs/src/styles/cinema-dark.css` and `pdf-document.tsx` — new tokens added here are **web-app-only** and need no mirroring (PDF excluded from scope). Do not edit the PDF files.
- Typecheck: `bun --filter=@luna-web/app run typecheck`. Tests: `bun --filter=@luna-web/app run test`.
- Commit after every task, conventional-commit style, ending with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- Live verification uses the in-app browser preview (`preview_start` name `luna-web`, port 5173). The recent folder **TEST_PROJECT_LUNA** (23 clips, 3 reels: `1`, `CAMERA`, `M001R00H`) re-scans without a new permission prompt — click it on the home page to exercise summary → processing → report.
- ZeroVer: version stays 0.x. Final task runs the `luna-release` skill; never propose 1.0.

---

### Task 1: Design tokens — dim text token, 2xs type token, prune dead sidebar tokens

**Files:**
- Modify: `apps/web/src/index.css`
- Modify: `apps/web/src/components/stat-tile.tsx:28`
- Modify: `apps/web/src/features/report/clip-card.tsx:85,89,150,173`
- Modify: `apps/web/src/features/scan/clip-tile.tsx:47`

**Interfaces:**
- Produces: Tailwind utilities `text-muted-foreground-dim` and `text-2xs` used by later tasks (Task 9 uses both).

- [ ] **Step 1: Add tokens to `index.css`**

In the `@theme inline` block, delete the eight `--color-sidebar-*` lines (13–20) and add after the `--color-muted-foreground` line:

```css
    --color-muted-foreground-dim: var(--muted-foreground-dim);
    --text-2xs: 0.7rem;
    --text-2xs--line-height: 1rem;
```

In **both** `:root` and `.dark` blocks: delete the eight `--sidebar*` custom properties, and add after `--muted-foreground`:

```css
    --muted-foreground-dim: oklch(0.61 0.01 285);
```

(Target: ≥4.5:1 on `--card` (L 0.17). Verify in Step 4 and nudge L up if it computes below 4.5.)

- [ ] **Step 2: Replace the failing `/70` usages**

`stat-tile.tsx:28`: `dim && 'text-muted-foreground/70'` → `dim && 'text-muted-foreground-dim'`.

`clip-card.tsx:89`: `text-muted-foreground/70 text-xs italic` → `text-muted-foreground-dim text-xs italic`.

- [ ] **Step 3: Replace ad-hoc micro sizes with `text-2xs`**

- `clip-card.tsx:85`: `text-[0.65rem]` → `text-2xs`
- `clip-card.tsx:150`: `text-[0.7rem]` → `text-2xs`
- `clip-card.tsx:173`: `text-[0.6rem]` → `text-2xs`
- `clip-tile.tsx:47`: `text-[0.7rem]` → `text-2xs`

- [ ] **Step 4: Verify**

Run: `bun --filter=@luna-web/app run typecheck` → PASS.
Start preview (`luna-web`), open http://localhost:5173, and in the browser JS console compute the contrast of `--muted-foreground-dim` on `--card` (canvas-convert oklch → rgb, WCAG formula, as in the audit script). Expected: ≥ 4.5. If below, raise L in 0.01 steps and re-check.
Grep guard: `text-\[0\.[67]` and `text-muted-foreground/70` and `--sidebar` must return no hits in `apps/web/src`.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/index.css apps/web/src/components/stat-tile.tsx apps/web/src/features/report/clip-card.tsx apps/web/src/features/scan/clip-tile.tsx
git commit -m "fix(web): AA-passing dim text token, unified 2xs type token, prune dead sidebar tokens"
```

---

### Task 2: `lib/format` — decimal byte units, local-time todayIso (TDD)

**Files:**
- Modify: `apps/web/src/lib/format.ts`
- Test: `apps/web/src/lib/format.test.ts`

**Interfaces:**
- Produces: `formatBytes(n: number): string` (decimal KB/MB/GB/TB), `todayIso(): string` (local calendar date). Signatures unchanged — all call sites keep working.

- [ ] **Step 1: Write failing tests** (append to `format.test.ts`)

```ts
import { expect, test } from 'bun:test'
import { formatBytes, todayIso } from './format'

test('formatBytes uses decimal units matching the labels', () => {
  expect(formatBytes(999)).toBe('999 B')
  expect(formatBytes(1000)).toBe('1.0 KB')
  expect(formatBytes(1_500_000)).toBe('1.5 MB')
  expect(formatBytes(37_200_000_000)).toBe('37.2 GB')
})

test('todayIso matches the local calendar date', () => {
  const now = new Date()
  const expected = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`
  expect(todayIso()).toBe(expected)
})
```

(The existing `import { todayIso } from './format'` line already exists — merge imports, don't duplicate.)

- [ ] **Step 2: Run to verify failure**

Run: `bun --filter=@luna-web/app run test`
Expected: FAIL — `formatBytes(1000)` returns `"1000 B"` under the current 1024 threshold; `todayIso` may fail near midnight UTC.

- [ ] **Step 3: Implement**

```ts
const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'] as const

export function formatBytes(n: number): string {
  if (n < 1000) return `${n} B`
  let value = n
  let unit = 0
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000
    unit += 1
  }
  return `${value.toFixed(1)} ${UNITS[unit]}`
}

/** Today as YYYY-MM-DD in local time — the cover-date default and export-filename stamp. */
export function todayIso(): string {
  const now = new Date()
  const mm = String(now.getMonth() + 1).padStart(2, '0')
  const dd = String(now.getDate()).padStart(2, '0')
  return `${now.getFullYear()}-${mm}-${dd}`
}
```

(`formatDuration` unchanged. Decimal units chosen to match the KB/MB/GB labels, Finder, and camera-card marketing sizes.)

- [ ] **Step 4: Run tests** — Expected: PASS (all).

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/lib/format.ts apps/web/src/lib/format.test.ts
git commit -m "fix(web): decimal byte units to match KB/MB/GB labels; todayIso uses local date"
```

---

### Task 3: Nav active state

**Files:**
- Modify: `apps/web/src/components/app-shell.tsx`

- [ ] **Step 1: Style the router's active state**

TanStack Router already stamps `data-status="active"` + `aria-current="page"` on the matching `<Link>`. In `AppShell`, define once above `return`:

```tsx
const navLink = cn(
  buttonVariants({ variant: 'ghost', size: 'sm' }),
  'data-[status=active]:bg-secondary data-[status=active]:text-foreground',
)
```

Use `className={navLink}` on the three router `<Link>`s (Reports, Settings, Activity). The `/docs/` `<a>` and version link keep their current classes (no router state on plain anchors).

- [ ] **Step 2: Verify live**

Preview: navigate to /reports/, /settings/, /activity/ — the matching nav item must show the secondary background + full-brightness text; others stay muted. Home shows none active (no nav item maps to `/`).

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/app-shell.tsx
git commit -m "feat(web): highlight the current page in the header nav"
```

---

### Task 4: Install shadcn primitives; replace hand-rolled controls

**Files:**
- Create (via CLI): `apps/web/src/components/ui/checkbox.tsx`, `alert-dialog.tsx`, `toggle-group.tsx` (+ `toggle.tsx` if the registry pulls it as a dependency)
- Modify: `apps/web/src/features/settings/settings-screen.tsx`
- Modify: `apps/web/src/features/reports/report-library-screen.tsx`
- Modify: `apps/web/src/features/activity/activity-screen.tsx`
- Modify: `apps/web/src/features/scan/scan-screen.tsx`

**Interfaces:**
- Produces: `ui/toggle-group` used again by Task 10's density toggle.

- [ ] **Step 1: Install via CLI**

```bash
cd apps/web && bunx shadcn@latest add checkbox alert-dialog toggle-group
```

Then **read each generated file** to learn the base-nova prop API (Base UI: `render={<Component />}` instead of Radix `asChild`; ToggleGroup value shape may be an array). All snippets below must be adapted to what the generated files actually export.

- [ ] **Step 2: Settings — Checkbox replaces the native checkbox** (`settings-screen.tsx:90-98`)

```tsx
<Label className="text-muted-foreground flex w-fit items-center gap-2 text-sm font-normal">
  <Checkbox
    checked={generateThumbnails}
    onCheckedChange={(checked) => void updateSettings({ generateThumbnails: checked === true })}
  />
  Generate thumbnails by default (override per run on the scan summary)
</Label>
```

- [ ] **Step 3: Settings — AlertDialog replaces inline clear-data confirm** (`settings-screen.tsx:193-222`)

Delete `confirmingClear` state. Keep `clearing`.

```tsx
<AlertDialog>
  <AlertDialogTrigger render={<Button variant="outline" size="sm" />}>
    Clear local data…
  </AlertDialogTrigger>
  <AlertDialogContent>
    <AlertDialogHeader>
      <AlertDialogTitle>Clear all local data?</AlertDialogTitle>
      <AlertDialogDescription>
        Deletes settings, report defaults, recent folders, the activity log, saved reports, and the
        cached decode engine on this device, then reloads Luna.
      </AlertDialogDescription>
    </AlertDialogHeader>
    <AlertDialogFooter>
      <AlertDialogCancel disabled={clearing}>Cancel</AlertDialogCancel>
      <AlertDialogAction
        className={cn(buttonVariants({ variant: 'destructive' }))}
        disabled={clearing}
        onClick={(e) => {
          e.preventDefault() // keep the dialog open while clearing runs; page reloads on completion
          setClearing(true)
          void clearLocalData()
        }}
      >
        {clearing ? 'Deleting…' : 'Delete everything'}
      </AlertDialogAction>
    </AlertDialogFooter>
  </AlertDialogContent>
</AlertDialog>
```

(If the generated AlertDialogAction already accepts a `variant` prop, use that instead of the `className` override.)

- [ ] **Step 4: Report library — AlertDialog replaces inline delete confirm** (`report-library-screen.tsx:67-96`)

Delete `confirmingId` state. Each row's trash button becomes a trigger:

```tsx
<AlertDialog>
  <AlertDialogTrigger
    render={<Button variant="ghost" size="icon-sm" aria-label={`Delete saved report ${s.title}`} />}
  >
    <Trash2 />
  </AlertDialogTrigger>
  <AlertDialogContent>
    <AlertDialogHeader>
      <AlertDialogTitle>Delete "{s.title}"?</AlertDialogTitle>
      <AlertDialogDescription>
        Removes this saved report{s.hasThumbnails ? ' and its stored thumbnails' : ''} from this
        device. This can't be undone.
      </AlertDialogDescription>
    </AlertDialogHeader>
    <AlertDialogFooter>
      <AlertDialogCancel>Cancel</AlertDialogCancel>
      <AlertDialogAction
        className={cn(buttonVariants({ variant: 'destructive' }))}
        onClick={() => {
          void deleteReport(s.id).then(() => listReportSummaries().then(setSummaries))
        }}
      >
        Delete
      </AlertDialogAction>
    </AlertDialogFooter>
  </AlertDialogContent>
</AlertDialog>
```

- [ ] **Step 5: Activity — ToggleGroup replaces filter buttons** (`activity-screen.tsx:73-82`)

```tsx
<ToggleGroup
  type="single"
  value={minLevel}
  onValueChange={(v) => {
    if (v) setMinLevel(v as LogLevel)
  }}
  aria-label="Minimum log level"
>
  {FILTERS.map((f) => (
    <ToggleGroupItem key={f.min} value={f.min}>
      {f.label}
    </ToggleGroupItem>
  ))}
</ToggleGroup>
```

(Adapt `type`/`value` to the generated component: base-nova ToggleGroup may model single-select as `value: string[]` + `toggleMultiple={false}` — inspect the file. Never allow deselect-to-empty: ignore empty values.)

- [ ] **Step 6: Scan summary — ToggleGroup replaces the hand-rolled On/Off fieldset** (`scan-screen.tsx:141-166`)

```tsx
<ToggleGroup
  type="single"
  value={generateThumbnails ? 'on' : 'off'}
  onValueChange={(v) => {
    if (v) scanStore.setState((s) => ({ ...s, generateThumbnails: v === 'on' }))
  }}
  aria-label="Thumbnails"
  className="bg-secondary shrink-0 gap-0.5 rounded-lg border p-0.5"
>
  <ToggleGroupItem
    value="on"
    className="data-[state=on]:bg-primary data-[state=on]:text-primary-foreground rounded-md px-3"
  >
    On
  </ToggleGroupItem>
  <ToggleGroupItem
    value="off"
    className="data-[state=on]:bg-primary data-[state=on]:text-primary-foreground rounded-md px-3"
  >
    Off
  </ToggleGroupItem>
</ToggleGroup>
```

(Adapt the `data-[state=on]` selector to whatever state attribute the generated item uses — Base UI may use `data-pressed` or `aria-pressed`.)

- [ ] **Step 7: Verify**

`bun --filter=@luna-web/app run typecheck` → PASS. Live: settings checkbox toggles and persists; "Clear local data…" opens a modal with Esc/backdrop dismiss; library trash opens a modal; activity filters and scan On/Off behave as before (re-scan TEST_PROJECT_LUNA for the scan summary). Keyboard: Tab reaches every new control; Esc closes dialogs.

- [ ] **Step 8: Commit**

```bash
git add apps/web/src/components/ui apps/web/src/features/settings/settings-screen.tsx apps/web/src/features/reports/report-library-screen.tsx apps/web/src/features/activity/activity-screen.tsx apps/web/src/features/scan/scan-screen.tsx apps/web/package.json bun.lock
git commit -m "refactor(web): shadcn checkbox, alert-dialog, toggle-group replace hand-rolled controls"
```

---

### Task 5: Heading semantics

**Files:**
- Modify: `apps/web/src/components/ui/card.tsx:36-47`
- Modify: `apps/web/src/features/scan/scan-screen.tsx:97`
- Modify: `apps/web/src/features/scan/processing-view.tsx:25`

- [ ] **Step 1: CardTitle renders `<h2>`**

In `card.tsx`, change `CardTitle` from `React.ComponentProps<"div">` + `<div>` to:

```tsx
function CardTitle({ className, ...props }: React.ComponentProps<"h2">) {
  return (
    <h2
      data-slot="card-title"
      className={cn(
        "font-heading text-base leading-snug font-medium group-data-[size=sm]/card:text-sm",
        className
      )}
      {...props}
    />
  )
}
```

Grep `CardTitle` across `apps/web/src` — current consumers are `settings-screen.tsx` and `cover-form.tsx`, both on pages whose h1 exists, so h2 is the correct level. If any other consumer appears, check its heading context.

- [ ] **Step 2: Scan summary gets an h1**

`scan-screen.tsx:97`: `<div className="truncate font-medium">{sourceName}</div>` → `<h1 className="truncate font-medium">{sourceName}</h1>`

- [ ] **Step 3: Processing phase heading becomes h1**

`processing-view.tsx:25`: `<h2 className="text-lg font-semibold" aria-live="polite">` → `<h1 className="text-lg font-semibold" aria-live="polite">`

- [ ] **Step 4: Verify** — typecheck PASS; in the preview a11y tree, Settings shows `heading "Settings"` then `heading` nodes for Processing / Report defaults / Local data; scan summary and processing phases each expose exactly one h1.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/components/ui/card.tsx apps/web/src/features/scan/scan-screen.tsx apps/web/src/features/scan/processing-view.tsx
git commit -m "fix(web): real heading elements for card titles and scan-phase titles"
```

---

### Task 6: Shared LogoDropWell; kill the native file input in Settings

**Files:**
- Create: `apps/web/src/components/logo-drop-well.tsx`
- Modify: `apps/web/src/features/report/cover-form.tsx` (delete its private `LogoDropWell`, import the shared one)
- Modify: `apps/web/src/features/settings/settings-screen.tsx:126-156`

**Interfaces:**
- Produces: `LogoDropWell({ id, value, onChange, className? })` — `value: Blob | undefined`, `onChange(file: Blob | undefined)`. `onChange(undefined)` means "remove".

- [ ] **Step 1: Create the shared component** — lift `cover-form.tsx:84-151` verbatim into `components/logo-drop-well.tsx`, parameterized:

```tsx
import { ImageUp } from 'lucide-react'
import { useRef, useState } from 'react'
import { useObjectUrl } from '@/lib/use-object-url'
import { cn } from '@/lib/utils'

export function LogoDropWell({
  id,
  value,
  onChange,
  className,
}: {
  id: string
  value: Blob | undefined
  onChange: (file: Blob | undefined) => void
  className?: string
}) {
  const previewUrl = useObjectUrl(value)
  const [dragging, setDragging] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  const accept = (file: File | undefined) => {
    if (file?.type.startsWith('image/')) onChange(file)
  }

  return (
    <div className="flex flex-col gap-1.5">
      <button
        type="button"
        onClick={() => inputRef.current?.click()}
        onDragOver={(e) => {
          e.preventDefault()
          setDragging(true)
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={(e) => {
          e.preventDefault()
          setDragging(false)
          accept(e.dataTransfer.files?.[0])
        }}
        className={cn(
          'focus-visible:border-ring focus-visible:ring-ring/50 flex aspect-[4/3] w-full cursor-pointer flex-col items-center justify-center gap-1.5 rounded-lg border border-dashed p-3 text-center outline-none transition focus-visible:ring-3',
          dragging
            ? 'border-primary bg-primary/5'
            : 'border-border hover:border-input hover:bg-muted/30',
          className,
        )}
      >
        {previewUrl ? (
          <img
            src={previewUrl}
            alt="Report logo"
            className="max-h-full max-w-full object-contain"
          />
        ) : (
          <>
            <ImageUp className="text-muted-foreground size-5" />
            <span className="text-muted-foreground text-xs">Drop or click</span>
          </>
        )}
      </button>
      {previewUrl && (
        <button
          type="button"
          onClick={() => onChange(undefined)}
          className="text-muted-foreground hover:text-foreground text-xs underline-offset-2 hover:underline"
        >
          Remove logo
        </button>
      )}
      <input
        ref={inputRef}
        id={id}
        type="file"
        accept="image/*"
        className="hidden"
        onChange={(e) => accept(e.target.files?.[0])}
      />
    </div>
  )
}
```

- [ ] **Step 2: cover-form uses it** — delete the private `LogoDropWell` function and its now-unused imports (`ImageUp`, `useRef`/`useState` if unused, `useObjectUrl`); replace the call site with:

```tsx
<div className="flex flex-col gap-1.5">
  <Label htmlFor="cover-logo" className="text-muted-foreground">Logo</Label>
  <LogoDropWell
    id="cover-logo"
    value={useSelector(coverStore, (s) => s.logo)}
    onChange={(file) => setCoverFields({ logo: file ?? undefined })}
  />
</div>
```

(Hook rules: hoist the `useSelector` call to the top of `CoverForm` as `const logo = useSelector(coverStore, (s) => s.logo)` and pass `value={logo}` — never call hooks inline in JSX props.)

- [ ] **Step 3: Settings uses it** — replace the `Input type="file"` block (`settings-screen.tsx:126-156`), keeping the Label:

```tsx
<div className="flex flex-col gap-1.5">
  <Label htmlFor="default-logo">Logo</Label>
  <LogoDropWell
    id="default-logo"
    value={coverDefaults.logo}
    className="aspect-video"
    onChange={(file) => {
      if (file) {
        void updateSettings({
          coverDefaults: { ...settingsStore.state.coverDefaults, logo: file },
        })
      } else {
        const { logo: _drop, ...rest } = settingsStore.state.coverDefaults
        void updateSettings({ coverDefaults: rest })
      }
    }}
  />
</div>
```

The separate "Remove" ghost button and the `resetNonce` keying for the logo input are no longer needed (the well shows its own remove affordance and is fully controlled).

- [ ] **Step 4: Verify** — typecheck PASS. Live in Settings: drop/click sets a logo preview, "Remove logo" clears it, no native "Choose File" control remains anywhere (`grep -rn 'type="file"' apps/web/src` → only `logo-drop-well.tsx`'s hidden input). Cover form on a processed report still works identically.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/components/logo-drop-well.tsx apps/web/src/features/report/cover-form.tsx apps/web/src/features/settings/settings-screen.tsx
git commit -m "refactor(web): shared logo drop-well replaces raw native file input in settings"
```

---

### Task 7: Save-button icon; skeleton loading states

**Files:**
- Modify: `apps/web/src/features/report/save-report-button.tsx:44`
- Modify: `apps/web/src/features/reports/report-library-screen.tsx:38`
- Modify: `apps/web/src/features/reports/saved-report-screen.tsx:34`

- [ ] **Step 1: Lucide Check instead of `✓`**

```tsx
import { Check } from 'lucide-react'
...
{state === 'saving' ? (
  'Saving…'
) : state === 'saved' ? (
  <>
    <Check className="size-4" /> Saved
  </>
) : (
  'Save report'
)}
```

- [ ] **Step 2: Library list skeleton** — replace `summaries === null ? null :` with:

```tsx
summaries === null ? (
  <div className="flex flex-col gap-2" aria-hidden>
    {[0, 1, 2].map((i) => (
      <div
        key={i}
        className="bg-card flex items-center justify-between gap-3 rounded-lg border px-4 py-3"
      >
        <div className="min-w-0 flex-1 space-y-2">
          <Skeleton className="h-4 w-48" />
          <Skeleton className="h-3 w-72" />
        </div>
        <Skeleton className="h-5 w-28" />
      </div>
    ))}
  </div>
) : ...
```

Import `Skeleton` from `@/components/ui/skeleton`.

- [ ] **Step 3: Saved-report skeleton** — replace `if (state.status === 'loading') return null` with:

```tsx
if (state.status === 'loading') {
  return (
    <div className="flex flex-col gap-6" aria-hidden>
      <div className="space-y-2">
        <Skeleton className="h-3 w-24" />
        <Skeleton className="h-8 w-64" />
        <Skeleton className="h-4 w-80" />
      </div>
      <Skeleton className="h-28 w-full rounded-xl" />
      <Skeleton className="h-96 w-full rounded-xl" />
    </div>
  )
}
```

- [ ] **Step 4: Verify** — typecheck PASS. Live: hard-reload /reports/ and a saved-report URL (throttle to "Slow 4G" in devtools if the flash is too quick to see) — skeletons render before content; no blank frame. Save a report from a fresh run: button shows Lucide check + "Saved".

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/report/save-report-button.tsx apps/web/src/features/reports/report-library-screen.tsx apps/web/src/features/reports/saved-report-screen.tsx
git commit -m "fix(web): lucide check on save button; skeletons replace blank loading states"
```

---

### Task 8: Report masthead + sticky reel nav with scrollspy

**Files:**
- Modify: `apps/web/src/features/report/report-view.tsx`

**Interfaces:**
- Consumes: `formatBytes`/`formatDuration` (Task 2 semantics), `text-2xs` (Task 1).
- Produces: reel sections keep `id={`reel-${slug(reel.name)}`}` and gain `data-reel-section` — Task 11's virtual list lives inside these sections.

- [ ] **Step 1: Masthead** — replace the header block (lines 35–51):

```tsx
<header className="mb-6 flex flex-wrap items-start justify-between gap-4">
  <div className="min-w-0">
    <p className="text-muted-foreground text-xs font-medium tracking-wider uppercase">
      {eyebrow}
    </p>
    <h1 className="truncate text-3xl font-semibold tracking-tight">
      {model.cover.projectTitle || 'Camera report'}
    </h1>
    <p className="text-muted-foreground mt-1.5 font-mono text-sm tabular-nums">
      {model.cover.date} · {model.stats.cardCount}{' '}
      {model.stats.cardCount === 1 ? 'card' : 'cards'} · {model.stats.clipCount}{' '}
      {model.stats.clipCount === 1 ? 'clip' : 'clips'} ·{' '}
      {formatDuration(model.stats.totalDurationSeconds)} ·{' '}
      {formatBytes(model.stats.totalSizeBytes)}
    </p>
    {metaLine.length > 0 && (
      <p className="text-muted-foreground mt-0.5 text-sm">{metaLine.join('  ·  ')}</p>
    )}
  </div>
  <div className="flex shrink-0 items-center gap-3">
    <CoverLogo logo={model.cover.logo} />
    {actions}
  </div>
</header>
```

Remove `model.cover.date` from the `metaLine` array (it now leads the mono line). Keep the 5-tile stat card unchanged — the mono line is the at-a-glance summary; the tiles remain the visual anchors.

- [ ] **Step 2: Scrollspy state** — inside `ReportView`:

```tsx
const [activeReel, setActiveReel] = useState<string | null>(null)

useEffect(() => {
  const sections = document.querySelectorAll('[data-reel-section]')
  if (sections.length < 2) return
  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) setActiveReel(entry.target.id)
      }
    },
    // Band across the upper-middle viewport: the reel whose content crosses it is "current".
    { rootMargin: '-30% 0px -60% 0px' },
  )
  for (const s of sections) observer.observe(s)
  return () => observer.disconnect()
}, [model])
```

Add imports: `useEffect, useState` from react, `cn` from `@/lib/utils`.

- [ ] **Step 3: Sticky reel bar** — replace the `multiReel` nav (lines 67–82):

```tsx
{multiReel && (
  <nav
    aria-label="Reels"
    className="bg-background/80 sticky top-14 z-20 -mx-2 mb-6 flex flex-wrap gap-2 border-b px-2 py-2 backdrop-blur"
  >
    {model.reels.map((reel) => {
      const id = `reel-${slug(reel.name)}`
      const isActive = activeReel === id
      return (
        <a
          key={reel.name}
          href={`#${id}`}
          aria-current={isActive ? 'true' : undefined}
          className={cn(
            'flex items-center gap-2 rounded-full border px-3 py-1 text-sm transition-colors outline-none focus-visible:ring-3 focus-visible:ring-ring/50',
            isActive
              ? 'border-primary/40 bg-primary/10 text-foreground'
              : 'bg-card hover:border-input',
          )}
        >
          <span className="font-medium">{reel.name}</span>
          <span className="text-muted-foreground font-mono tabular-nums">
            {reel.stats.clipCount}
          </span>
        </a>
      )
    })}
  </nav>
)}
```

- [ ] **Step 4: Offset the reel headers below the sticky bar** — on each reel `<section>` add `data-reel-section` and make offsets conditional:

```tsx
<section
  key={reel.name}
  id={`reel-${slug(reel.name)}`}
  data-reel-section
  className={cn('mb-8', multiReel ? 'scroll-mt-[7rem]' : 'scroll-mt-16')}
>
  <div
    className={cn(
      'bg-background/80 sticky z-10 mb-3 flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 border-b py-2 backdrop-blur',
      multiReel ? 'top-[6.5rem]' : 'top-14',
    )}
  >
```

(`6.5rem` = header `3.5rem` + reel bar ≈ `3rem`. Verify live and adjust ±0.25rem so the reel header docks flush under the pill bar with no gap or overlap.)

- [ ] **Step 5: Verify** — typecheck PASS. Live with TEST_PROJECT_LUNA processed: pill bar stays docked while scrolling all 10k px; the pill for the reel under the reading band lights up as you pass `1` → `CAMERA` → `M001R00H`; clicking a pill jumps with the section title visible (not hidden under the bars); Tab reaches pills with the cyan ring; masthead shows `2026-07-20 · 1 card · 23 clips · 5:03 · 39.9 GB` (decimal size per Task 2).

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/features/report/report-view.tsx
git commit -m "feat(web): report masthead and sticky reel nav with scrollspy"
```

---

### Task 9: Clip card — flowing metadata list with tooltips

**Files:**
- Modify: `apps/web/src/features/report/clip-card.tsx`

**Interfaces:**
- Consumes: `ui/tooltip` (already present), `text-2xs` + `text-muted-foreground-dim` (Task 1).
- Produces: `ClipCard` signature unchanged — Tasks 10/11 render it as-is.

- [ ] **Step 1: Read `ui/tooltip.tsx`** to confirm the base-nova API (expect `Tooltip`, `TooltipTrigger`, `TooltipContent`, possibly a `TooltipProvider`; Base UI composition via `render`). Adapt Step 2 to it.

- [ ] **Step 2: Replace the two `MetaColumn`s with one flowing `MetaList`**

Delete `MetaColumn`. Add:

```tsx
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'

function MetaRow({ icon: Icon, label, value }: { icon: LucideIcon; label: string; value: string }) {
  return (
    <div className="border-border/40 flex items-baseline justify-between gap-3 border-b py-1 text-sm break-inside-avoid">
      <dt className="text-muted-foreground flex items-center gap-1.5">
        <Icon className="size-3.5 shrink-0" />
        {label}
      </dt>
      <dd className="min-w-0">
        <Tooltip>
          <TooltipTrigger
            render={<span className="block max-w-56 truncate text-right font-mono tabular-nums" />}
          >
            {value}
          </TooltipTrigger>
          <TooltipContent className="font-mono">{value}</TooltipContent>
        </Tooltip>
      </dd>
    </div>
  )
}

function MetaList({ clip }: { clip: ReportClip<Blob> }) {
  const groups = [
    { title: 'Technical', rows: technicalRows(clip).filter((r) => r.value), emptyNote: undefined },
    {
      title: 'Camera',
      rows: cameraRows(clip).filter((r) => r.value),
      emptyNote: 'No camera metadata in this file',
    },
  ] as const
  return (
    <div className="gap-x-6 self-start sm:columns-2">
      {groups.map(({ title, rows, emptyNote }) => (
        <section key={title} className="mb-3 break-inside-avoid-column">
          <div className="text-muted-foreground mb-2 text-2xs font-medium tracking-wider uppercase">
            {title}
          </div>
          {rows.length === 0 ? (
            <p className="text-muted-foreground-dim text-xs italic">{emptyNote ?? '—'}</p>
          ) : (
            <dl>
              {rows.map((row) => (
                <MetaRow key={row.label} icon={row.icon} label={row.label} value={row.value as string} />
              ))}
            </dl>
          )}
        </section>
      ))}
    </div>
  )
}
```

CSS-columns note: if `break-inside-avoid-column` on whole sections makes the two groups refuse to balance (Camera with 8 rows forced into one column), drop the class from `<section>` and keep `break-inside-avoid` only on rows so rows flow freely across both columns under their running group headers. Judge live with an ALEXA Mini clip (8 camera fields) and the BRAW clip (2 fields).

- [ ] **Step 3: Use it** — in `ClipCard`, replace the `grid grid-cols-2 gap-x-6 gap-y-4 self-start` div and its two `MetaColumn`s with `<MetaList clip={clip} />`.

- [ ] **Step 4: Frame-thumbnail focus rings** — in `FrameViewer`'s thumb `<button>` `cn(...)`, add `'outline-none focus-visible:ring-3 focus-visible:ring-ring/50'` to the base class string.

- [ ] **Step 5: Verify** — typecheck PASS. Live on TEST_PROJECT_LUNA: the VENICE/BURANO clips (long lens names like "Fujinon Alura AZ…") show a tooltip with the full value on hover **and** on keyboard focus; the two-field BRAW clip no longer leaves a half-empty right column; Tab shows rings on frame thumbs.

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/features/report/clip-card.tsx
git commit -m "feat(web): flowing clip metadata list with full-value tooltips"
```

---

### Task 10: Density toggle — card view / table view

**Files:**
- Create: `apps/web/src/features/report/clip-table.tsx`
- Modify: `apps/web/src/features/report/report-view.tsx`

**Interfaces:**
- Consumes: `ui/toggle-group` (Task 4), `ui/table`, `formatBytes`/`formatDuration`.
- Produces: `ClipTable({ clips }: { clips: ReportClip<Blob>[] })`.

- [ ] **Step 1: Create `clip-table.tsx`**

```tsx
import type { ReportClip } from '@luna-web/core'
import { Card } from '@/components/ui/card'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { formatBytes, formatDuration } from '@/lib/format'

export function ClipTable({ clips }: { clips: ReportClip<Blob>[] }) {
  return (
    <Card className="overflow-hidden py-0">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="px-4">File</TableHead>
            <TableHead>TC</TableHead>
            <TableHead>Resolution</TableHead>
            <TableHead>Codec</TableHead>
            <TableHead className="text-right">FPS</TableHead>
            <TableHead className="text-right">Duration</TableHead>
            <TableHead className="text-right">ISO</TableHead>
            <TableHead>WB</TableHead>
            <TableHead>Lens</TableHead>
            <TableHead className="px-4 text-right">Size</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {clips.map((clip) => {
            const m = clip.metadata
            return (
              <TableRow key={clip.id}>
                <TableCell className="max-w-64 truncate px-4 font-mono font-medium" title={clip.fileName}>
                  {clip.fileName}
                </TableCell>
                <TableCell className="font-mono tabular-nums">{m.startTimecode ?? '—'}</TableCell>
                <TableCell className="font-mono tabular-nums">
                  {m.width && m.height ? `${m.width}×${m.height}` : '—'}
                </TableCell>
                <TableCell>{m.codec ?? '—'}</TableCell>
                <TableCell className="text-right font-mono tabular-nums">
                  {m.frameRate ?? '—'}
                </TableCell>
                <TableCell className="text-right font-mono tabular-nums">
                  {m.durationSeconds !== undefined ? formatDuration(m.durationSeconds) : '—'}
                </TableCell>
                <TableCell className="text-right font-mono tabular-nums">{m.iso ?? '—'}</TableCell>
                <TableCell className="font-mono tabular-nums">{m.whiteBalance ?? '—'}</TableCell>
                <TableCell className="max-w-40 truncate" title={m.lens}>
                  {m.lens ?? '—'}
                </TableCell>
                <TableCell className="px-4 text-right font-mono tabular-nums">
                  {formatBytes(clip.sizeBytes)}
                </TableCell>
              </TableRow>
            )
          })}
        </TableBody>
      </Table>
    </Card>
  )
}
```

(Check `ClipMetadata` field types in `@luna-web/core` — if `iso`/`whiteBalance`/`frameRate` need string coercion, render with `String(...)`.)

- [ ] **Step 2: Density state + toggle in `ReportView`**

```tsx
const [density, setDensity] = useState<'cards' | 'table'>('cards')
```

Place the toggle at the right end of the stats card row — directly above the first reel, after `{children}`:

```tsx
import { LayoutGrid, Rows3 } from 'lucide-react'

<div className="mb-3 flex justify-end">
  <ToggleGroup
    type="single"
    value={density}
    onValueChange={(v) => {
      if (v) setDensity(v as 'cards' | 'table')
    }}
    aria-label="Clip display density"
  >
    <ToggleGroupItem value="cards">
      <LayoutGrid className="size-4" /> Cards
    </ToggleGroupItem>
    <ToggleGroupItem value="table">
      <Rows3 className="size-4" /> Table
    </ToggleGroupItem>
  </ToggleGroup>
</div>
```

- [ ] **Step 3: Branch per reel** — replace the clip grid inside the reel loop:

```tsx
{density === 'cards' ? (
  <div className="grid gap-4">
    {reel.clips.map((clip) => (
      <ClipCard key={clip.id} clip={clip} sourceRoot={model.sourceRoot} />
    ))}
  </div>
) : (
  <ClipTable clips={reel.clips} />
)}
```

(Task 11 swaps the cards branch for the virtualized list — keep this exact shape so the swap is one-line.)

- [ ] **Step 4: Verify** — typecheck PASS. Live: toggle flips all reels between cards and a dense mono table; table scrolls horizontally inside its card at narrow widths (body never scrolls horizontally); tabular-nums keeps digit columns aligned; toggle keyboard-operable.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/report/clip-table.tsx apps/web/src/features/report/report-view.tsx
git commit -m "feat(web): card/table density toggle for report clips"
```

---

### Task 11: Virtualized clip card lists

**Files:**
- Create: `apps/web/src/features/report/virtual-clip-list.tsx`
- Modify: `apps/web/src/features/report/report-view.tsx` (cards branch)
- Modify: `apps/web/package.json` (dep)

**Interfaces:**
- Consumes: `ClipCard` (unchanged signature).
- Produces: `VirtualClipList({ clips, sourceRoot })`.

- [ ] **Step 1: Install**

```bash
cd apps/web && bun add @tanstack/react-virtual
```

- [ ] **Step 2: Create `virtual-clip-list.tsx`**

```tsx
import type { ReportClip } from '@luna-web/core'
import { useWindowVirtualizer } from '@tanstack/react-virtual'
import { useLayoutEffect, useRef, useState } from 'react'
import { ClipCard } from './clip-card'

// Below this count plain rendering is cheaper than absolute positioning.
const VIRTUALIZE_THRESHOLD = 15

export function VirtualClipList({
  clips,
  sourceRoot,
}: {
  clips: ReportClip<Blob>[]
  sourceRoot: string
}) {
  if (clips.length <= VIRTUALIZE_THRESHOLD) {
    return (
      <div className="grid gap-4">
        {clips.map((clip) => (
          <ClipCard key={clip.id} clip={clip} sourceRoot={sourceRoot} />
        ))}
      </div>
    )
  }
  return <WindowedClipList clips={clips} sourceRoot={sourceRoot} />
}

function WindowedClipList({
  clips,
  sourceRoot,
}: {
  clips: ReportClip<Blob>[]
  sourceRoot: string
}) {
  const listRef = useRef<HTMLDivElement>(null)
  const [scrollMargin, setScrollMargin] = useState(0)

  // The window virtualizer needs the list's offset from the top of the page.
  // Content above (cover form, other reels) can change height, so track it.
  useLayoutEffect(() => {
    const el = listRef.current
    if (!el) return
    const update = () => setScrollMargin(el.getBoundingClientRect().top + window.scrollY)
    update()
    const observer = new ResizeObserver(update)
    observer.observe(document.body)
    return () => observer.disconnect()
  }, [])

  const virtualizer = useWindowVirtualizer({
    count: clips.length,
    estimateSize: () => 480,
    overscan: 3,
    gap: 16,
    scrollMargin,
    getItemKey: (i) => clips[i]?.id ?? i,
  })

  return (
    <div ref={listRef} className="relative" style={{ height: virtualizer.getTotalSize() }}>
      {virtualizer.getVirtualItems().map((item) => (
        <div
          key={item.key}
          data-index={item.index}
          ref={virtualizer.measureElement}
          className="absolute inset-x-0 top-0"
          style={{ transform: `translateY(${item.start - virtualizer.options.scrollMargin}px)` }}
        >
          <ClipCard clip={clips[item.index]} sourceRoot={sourceRoot} />
        </div>
      ))}
    </div>
  )
}
```

- [ ] **Step 3: Swap into `report-view.tsx`** — the cards branch from Task 10 becomes:

```tsx
{density === 'cards' ? (
  <VirtualClipList clips={reel.clips} sourceRoot={model.sourceRoot} />
) : (
  <ClipTable clips={reel.clips} />
)}
```

Remove the now-unused direct `ClipCard` import if nothing else uses it in this file.

- [ ] **Step 4: Verify** — typecheck PASS. Live with TEST_PROJECT_LUNA: the 19-clip CAMERA reel virtualizes (in the browser console, `document.querySelectorAll('[data-slot=card]').length` while scrolled mid-reel is well under the total clip count; blob image count drops similarly); scrolling fast through the reel shows no blank flashes or overlap; frame thumbnails still cycle; reel-pill jumps still land correctly; scrollspy still tracks. The 1-clip and 3-clip reels render the plain grid.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/report/virtual-clip-list.tsx apps/web/src/features/report/report-view.tsx apps/web/package.json bun.lock
git commit -m "feat(web): virtualize large clip lists with @tanstack/react-virtual"
```

---

### Task 12: Processing view — collapse completed, show in-flight

**Files:**
- Modify: `apps/web/src/features/scan/processing-view.tsx`

**Interfaces:**
- Consumes: `scanStore` state — `ClipProcessStatus = 'queued' | 'processing' | 'done' | 'failed'`, `ThumbStatus = 'queued' | 'decoding' | 'done' | 'failed'`, `thumbsById`, `clipErrors`/`thumbErrors`. `ClipTile` unchanged for in-flight/failed tiles.

- [ ] **Step 1: Rewrite `ProcessingView`**

```tsx
import type { ClipRef } from '@luna-web/core'
import { useSelector } from '@tanstack/react-store'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { useObjectUrl } from '@/lib/use-object-url'
import { ClipTile } from './clip-tile'
import { resetScan } from './run-scan'
import { scanStore } from './store'

export function ProcessingView() {
  const phase = useSelector(scanStore, (s) => s.phase)
  const clips = useSelector(scanStore, (s) => s.clips)
  const clipStatus = useSelector(scanStore, (s) => s.clipStatus)
  const thumbStatus = useSelector(scanStore, (s) => s.thumbStatus)
  const processedCount = useSelector(scanStore, (s) => s.processedCount)
  const thumbDoneCount = useSelector(scanStore, (s) => s.thumbDoneCount)

  const inThumb = phase === 'thumbnailing'
  const statusOf = (id: string) =>
    inThumb ? (thumbStatus[id] ?? 'queued') : (clipStatus[id] ?? 'queued')

  const active = clips.filter((c) => {
    const s = statusOf(c.id)
    return s === 'processing' || s === 'decoding'
  })
  const failed = clips.filter((c) => statusOf(c.id) === 'failed')
  const doneClips = clips.filter((c) => statusOf(c.id) === 'done')
  const queuedCount = clips.length - active.length - failed.length - doneClips.length

  const done = inThumb ? thumbDoneCount : processedCount
  const total = inThumb ? Object.keys(thumbStatus).length || clips.length : clips.length
  const pct = total > 0 ? (done / total) * 100 : 0

  return (
    <section className="space-y-5">
      <div className="space-y-3">
        <div className="flex items-end justify-between gap-4">
          <div>
            <h1 className="text-lg font-semibold" aria-live="polite">
              {inThumb ? 'Generating thumbnails' : 'Reading metadata'}
            </h1>
            <p className="text-muted-foreground font-mono text-sm tabular-nums">
              {done} / {total} clips
              {queuedCount > 0 && ` · ${queuedCount} queued`}
            </p>
          </div>
          <Button variant="outline" size="sm" onClick={resetScan}>
            Start over
          </Button>
        </div>
        <Progress value={pct} />
      </div>

      {active.length > 0 && (
        <div>
          <h2 className="text-muted-foreground mb-2 text-xs font-medium tracking-wide uppercase">
            In progress
          </h2>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
            {active.map((clip) => (
              <ClipTile key={clip.id} clip={clip} />
            ))}
          </div>
        </div>
      )}

      {doneClips.length > 0 && (
        <div>
          <h2 className="text-muted-foreground mb-2 text-xs font-medium tracking-wide uppercase">
            Completed
            <span className="ml-1.5 font-mono tabular-nums">{doneClips.length}</span>
          </h2>
          <div className="flex flex-wrap gap-1.5">
            {doneClips.slice(-12).map((clip) => (
              <DoneThumb key={clip.id} clip={clip} />
            ))}
            {doneClips.length > 12 && (
              <span className="text-muted-foreground self-center font-mono text-xs tabular-nums">
                +{doneClips.length - 12} more
              </span>
            )}
          </div>
        </div>
      )}

      {failed.length > 0 && (
        <div>
          <h2 className="text-destructive mb-2 text-xs font-medium tracking-wide uppercase">
            Failed
            <span className="ml-1.5 font-mono tabular-nums">{failed.length}</span>
          </h2>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
            {failed.map((clip) => (
              <ClipTile key={clip.id} clip={clip} />
            ))}
          </div>
        </div>
      )}
    </section>
  )
}

// Compact strip thumb for a finished clip — fades in as the worker completes it.
function DoneThumb({ clip }: { clip: ClipRef }) {
  const frames = useSelector(scanStore, (s) => s.thumbsById[clip.id])
  const first = frames?.find((f) => f.outcome === 'Success' && f.image)?.image
  const url = useObjectUrl(first)
  return (
    <div
      className="bg-card animate-in fade-in zoom-in-95 aspect-video w-16 overflow-hidden rounded border duration-200"
      title={clip.fileName}
    >
      {url ? (
        <img src={url} alt="" className="h-full w-full object-cover" />
      ) : (
        <div className="bg-muted/40 h-full w-full" />
      )}
    </div>
  )
}
```

Notes: the h1 also lands Task 5's heading fix for this file (if Task 5 already changed it, this rewrite preserves it). During the metadata phase there are no thumbnails yet — DoneThumb renders muted placeholders, which is correct (it reads as "counted, moving on"). Reduced motion is globally clamped by `index.css`, so `animate-in` needs no extra guard.

- [ ] **Step 2: Verify** — typecheck PASS. Live: re-run TEST_PROJECT_LUNA with thumbnails ON — DOM stays bounded (in-progress grid never exceeds the worker cap; completed strip holds ≤12 thumbs + a counter); thumbs fade in as workers finish; if any clip fails it stays visible under "Failed"; finishing lands on the report as before. Also run once with thumbnails OFF (metadata phase only).

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/scan/processing-view.tsx
git commit -m "feat(web): processing view collapses completed clips, spotlights in-flight work"
```

---

### Task 13: Focus-ring and list-row consistency sweep

**Files:**
- Modify: `apps/web/src/features/scan/recent-list.tsx:30-33`
- Modify: `apps/web/src/features/activity/activity-screen.tsx:126-128`

- [ ] **Step 1: Recent-folders row** — the folder `<button>` (line 31) gets the ring and the row gets library-row padding:

Row div: `px-3 py-2` → `px-4 py-3`.
Button className: `'flex min-w-0 items-center gap-2 text-left text-sm'` → `'flex min-w-0 items-center gap-2 rounded text-left text-sm outline-none focus-visible:ring-3 focus-visible:ring-ring/50'`

- [ ] **Step 2: Activity group collapse button** — add to its className: `rounded-lg outline-none focus-visible:ring-3 focus-visible:ring-ring/50`

- [ ] **Step 3: Verify** — typecheck PASS; Tab through home (with recent folder present) and an activity log with entries: every stop shows the cyan ring, no UA default outlines. Recent row height now matches the report-library rows.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/scan/recent-list.tsx apps/web/src/features/activity/activity-screen.tsx
git commit -m "fix(web): consistent focus rings and list-row padding on custom interactive elements"
```

---

### Task 14: Full verification sweep + release

**Files:**
- Modify: `CHANGELOG.md` + version files (via `luna-release` skill)

- [ ] **Step 1: Static gates**

```bash
bun --filter=@luna-web/app run typecheck
bun --filter=@luna-web/app run test
```

Expected: both PASS. Also run the repo's Biome check if a root script exists (`bun run check` — inspect root `package.json`).

- [ ] **Step 2: Live sweep** (preview `luna-web`, TEST_PROJECT_LUNA):
  - Home → summary → processing (collapse behavior) → report: masthead, sticky reel bar scrollspy, cards↔table toggle, tooltips, virtualization.
  - Settings: checkbox, logo drop-well, clear-data dialog. Reports library: skeleton, delete dialog. Activity: toggle-group filters.
  - Re-run the audit contrast script on the report page: no text node below 4.5:1.
  - A11y tree per route: exactly one h1; card section titles are headings; nav shows `aria-current`.
  - Mobile preset (375): no horizontal document scroll (table scrolls inside its own container).
  - Keyboard-only lap: Tab order sane, rings visible everywhere, Esc closes dialogs, Enter/Esc still work on scan summary.
- [ ] **Step 3: Screenshot proof** — home, scan summary, processing mid-run, report (cards), report (table), settings.
- [ ] **Step 4: Release** — invoke the `luna-release` skill: changelog entries for the user-facing changes (nav highlight, decimal sizes, table view, sticky reel nav, masthead, processing redesign, dialogs/controls polish, virtualization) and let it judge the ZeroVer minor bump.
- [ ] **Step 5: Final commit/push per the release skill's output.**

---

## Self-review notes

- Spec coverage: defects 1–6 → Tasks 3, 1, 5, 6, 9, 11. Consistency 1–6 → Tasks 4, 4, 7, 13, 1, 7(+2). Next-level 1–6 → Tasks 10, 8, 8, 12, 4(primitives, no toast), 9. Deferred by user decision: full value-wrap in clip cards (tooltip shipped instead — recorded in memory), light theme, gradient escalation, scan-summary rework.
- Base-nova prop APIs (AlertDialog/ToggleGroup/Tooltip/Checkbox) are registry-generated; every consuming step starts with "read the generated file and adapt" — that is deliberate, not a placeholder: the CLI output is the source of truth.
- Type consistency: `LogoDropWell` (Task 6) is consumed with the exact `{ id, value, onChange, className? }` signature in both call sites; `VirtualClipList`/`ClipTable` (Tasks 10–11) both take `clips: ReportClip<Blob>[]`; `density` state string union matches ToggleGroupItem values.
