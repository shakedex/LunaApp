# Running Luna from scripts and AI agents

Luna installs two programs side by side:

| Program | Window | Use |
|---|---|---|
| `Luna.exe report …` (`Luna.app/Contents/MacOS/Luna`) | Opens, already running | Watch or cancel the run |
| `luna-cli report …` | None | Scripts: result on stdout, exit code |
| `luna-cli inspect …` | None | List cameras, cards and clips (metadata only) |
| `luna-cli mcp` | None | Tools for AI agents (MCP over stdio) |

Installed paths:

- Windows: `C:\Program Files\Luna\luna-cli.exe` and `Luna.exe`
- macOS: `/Applications/Luna.app/Contents/MacOS/luna-cli` and `Luna`

## `report`

```
luna-cli report <SOURCE>... [options]
```

| Option | |
|---|---|
| `<SOURCE>...` | One or more folders, scanned recursively. One report covers all of them. |
| `--camera <NAME>` | Only this camera (repeatable). A letter (`A`, `B_CAM`, `cam c`) selects every body under it; a camera id from `inspect` selects that body. |
| `--production <TEXT>` | |
| `--company <TEXT>` | |
| `--author <TEXT>` | |
| `--logo <PATH>` | PNG, JPEG or SVG |
| `--field <LABEL=VALUE>` | Header field (repeatable), printed after the saved fields; a saved field with the same label takes this value |
| `--no-saved-fields` | Leave out the saved header fields |
| `--output <DIR>` | Folder the report folder is written into |
| `--name <TEXT>` | Report folder name (default: production name and run date) |
| `--theme <light\|dark>` | |
| `--json` | Print the result as JSON (`luna-cli` only) |

Options apply to that run only; saved Settings are never changed. Anything not
given comes from Settings. An existing report folder is never overwritten:
` (2)`, ` (3)` … is appended.

Progress goes to stderr. Without `--json`, stdout is the PDF path, then the
summary. With `--json`:

```json
{
  "ok": true,
  "report_dir": "D:\\Luna Reports\\Big Film 2026-10-16 1200",
  "files": { "pdf": "…", "html": "…", "csv": "…" },
  "clips": 142,
  "clips_with_problems": 2,
  "summary": "2 of 142 clips need attention",
  "not_included": 3,
  "unreadable": 0
}
```

On failure: `{"ok": false, "error": "…", "code": 3}`, and no files are written.

| Exit code | |
|---|---|
| 0 | Report written, no clip problems |
| 1 | Report written, some clips need attention |
| 2 | Bad arguments |
| 3 | A source folder can't be read |
| 4 | Report failed (output not writable, workers failed, no matching camera) |
| 5 | Cancelled (Ctrl+C) |

## `inspect`

```
luna-cli inspect <SOURCE>... [--json]
```

Reads metadata only, no frames, and prints one line per card. With `--json`:
`sources`, `cameras[]` (`id`, `letter`, `model`, `serial`, `cards[]` with
`reel`, `folder`, `clips`, `formats`, `first_recorded`, `last_recorded`,
`bytes`), `not_included`, `unreadable`. A camera `id` is accepted as is by
`--camera`.

## OffShoot

[`offshoot_luna.py`](offshoot_luna.py) makes a report each time OffShoot finishes
copying a card. Every destination gets its own report, and each opens its own
Luna window.

1. Copy `offshoot_luna.py` somewhere permanent. The constants at its top have
   defaults that work as installed:
   - `MODE`: `visible` (a Luna window) or `headless` (`luna-cli`, no window)
   - `DESTINATION_MATCH`: report only on destinations under this path (empty: every destination)
   - `OUTPUT`: where reports go (empty: Luna's saved location)
2. OffShoot: **Settings → Scripting → File Copy Completed**, add the script. Needs
   OffShoot Pro and Python 3.
3. Run one test offload. To see exactly what OffShoot sends, call the script with
   `--dump` once; it writes `offshoot_payload.json` next to itself.

Only copies that end in Success get a report. The card's source info from
OffShoot (Shoot Day, Date, Source Name, Location and any others) goes into the
report header under OffShoot's names; a saved field with the same name takes
OffShoot's value. Production, company and the logo come from Luna's Settings.

## Claude Desktop

Add Luna to `claude_desktop_config.json`
(Windows: `%APPDATA%\Claude\claude_desktop_config.json`, macOS:
`~/Library/Application Support/Claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "luna": {
      "command": "C:\\Program Files\\Luna\\luna-cli.exe",
      "args": ["mcp"]
    }
  }
}
```

On macOS, use `"/Applications/Luna.app/Contents/MacOS/luna-cli"` as the
command. Restart Claude Desktop.

Tools:

| Tool | Input | Returns |
|---|---|---|
| `inspect_folders` | `paths` | The `inspect` JSON |
| `start_report` | `sources`, optional `cameras`, `production`, `company`, `author`, `logo`, `fields` (`[{label, value}]`), `output`, `name`, `theme` | `report_id` at once |
| `report_status` | `report_id` | `state` (`queued`, `running`, `done`, `failed`, `cancelled`), progress counts, and the report result once ended |
| `cancel_report` | `report_id` | `state` |

Reports run one at a time in start order. Closing Claude Desktop cancels
any report still running; a cancelled report writes no files.
