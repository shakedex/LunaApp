# Luna's website

A static Astro site at https://luna.shaked.co, served from Cloudflare Workers
(static assets). Every picture on it is rendered by Luna's own code: the app's window and star chart
through its Slint UI and `luna-app` renderers, the sample report through
`luna-report`, and the Moon live in the browser through a GLSL port of
`crates/luna-app/src/moon.rs`.

## Commands

| Command             | What it does                                  |
| ------------------- | --------------------------------------------- |
| `bun install`       | Install dependencies                          |
| `bun run dev`       | Dev server                                    |
| `bun run check`     | Type check, tests and a production build      |
| `bun run build`     | Build into `dist/`                            |
| `bun run deploy`    | Build and `wrangler deploy`                   |

## Generated assets

`public/img` and `src/data` are generated. To rebuild them:

```bash
tools/frames.sh <dir with the GH5 clips> <frames dir>   # needs ffmpeg
tools/build-assets.sh <frames dir>                      # needs cargo, ffmpeg, python + pymupdf
```

`tools/capture` is a Rust program outside Luna's workspace that links
`luna-app` and `luna-report`; it builds into Luna's `target/` directory. It
writes the app's screens at 2x (`app-*@2x.webp`, with `layout.json` saying
where the Moon and the text lines are), the star chart (`sky.webp`), the
lunar albedo for the shader (`moon-albedo.webp`) and the sample report (PDF,
HTML, CSV; the pages become `report/*.webp`).

The sample report uses frames from Shaked's GH5 clips of Luna the cat with
made-up production metadata. Camera files never enter the repository.

## Copy

All visible text is in `src/copy.ts`.
