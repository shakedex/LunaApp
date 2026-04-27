using LunaApp.Models;

namespace LunaApp.Services.Reports.Html;

/// <summary>
/// Embedded CSS for HTML reports. Returns <c>&lt;style&gt;</c> blocks ready to drop
/// into the document <c>&lt;head&gt;</c>. Kept as raw strings (not a file resource)
/// so reports stay self-contained and portable.
///
/// Tokens adapt to <see cref="ReportTheme"/>: light for client-ready sharing/print,
/// dark for on-screen review. The <em>same</em> structure + class names render both
/// themes — only the CSS variables differ.
/// </summary>
internal static class ReportStylesheet
{
    public static string Base(ReportTheme theme) => $$"""
            <style>
                :root {
            {{Palette(theme)}}

                    --radius-sm: 4px;
                    --radius:    8px;
                    --radius-lg: 14px;

                    --shadow-sm: 0 1px 2px rgba(0,0,0,.05), 0 1px 1px rgba(0,0,0,.04);
                    --shadow:    0 1px 3px rgba(0,0,0,.08), 0 4px 12px rgba(0,0,0,.05);
                }

                * { margin: 0; padding: 0; box-sizing: border-box; }

                html { font-size: 15px; }
                body {
                    font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif;
                    font-feature-settings: 'ss01','cv11';
                    background: var(--bg);
                    color: var(--ink);
                    line-height: 1.55;
                    -webkit-font-smoothing: antialiased;
                    -moz-osx-font-smoothing: grayscale;
                }

                a { color: var(--accent); text-decoration: none; }
                a:hover { color: var(--accent-2); text-decoration: underline; }

                /* Report header (top of page 1) */
                .cover {
                    background: var(--surface);
                    border-bottom: 1px solid var(--line);
                    padding: 2.5rem 2rem 1.75rem;
                }
                .cover-inner {
                    max-width: 1200px;
                    margin: 0 auto;
                    display: grid;
                    grid-template-columns: 1fr auto;
                    gap: 2rem;
                    align-items: start;
                }
                .cover .eyebrow {
                    font-size: .72rem;
                    letter-spacing: .2em;
                    text-transform: uppercase;
                    color: var(--ink-muted);
                    font-weight: 600;
                    margin-bottom: .6rem;
                }
                .cover h1 {
                    font-size: 2.1rem;
                    font-weight: 700;
                    letter-spacing: -.015em;
                    color: var(--ink);
                    line-height: 1.15;
                    margin-bottom: .75rem;
                    word-break: break-word;
                }
                .cover .meta {
                    display: flex;
                    flex-wrap: wrap;
                    gap: .5rem 1rem;
                    color: var(--ink-2);
                    font-size: .92rem;
                }
                .cover .meta .dot { color: var(--ink-muted); }
                .cover .logo {
                    max-height: 80px;
                    max-width: 180px;
                    object-fit: contain;
                }

                /* Hero stats strip */
                .stats {
                    background: var(--surface);
                    border-bottom: 1px solid var(--line);
                    padding: 1rem 2rem;
                }
                .stats-inner {
                    max-width: 1200px;
                    margin: 0 auto;
                    display: flex;
                    gap: 2.5rem;
                    flex-wrap: wrap;
                }
                .stat {
                    display: flex;
                    flex-direction: column;
                    gap: .1rem;
                }
                .stat .num {
                    font-size: 1.55rem;
                    font-weight: 700;
                    letter-spacing: -.01em;
                    color: var(--ink);
                    font-variant-numeric: tabular-nums;
                }
                .stat .lbl {
                    font-size: .68rem;
                    font-weight: 600;
                    letter-spacing: .14em;
                    text-transform: uppercase;
                    color: var(--ink-muted);
                }

                main { max-width: 1200px; margin: 0 auto; padding: 2rem; }

                /* ============ Clip card ============ */
                .clip-card {
                    background: var(--surface);
                    border: 1px solid var(--line);
                    border-radius: var(--radius-lg);
                    overflow: hidden;
                    box-shadow: var(--shadow-sm);
                    margin-bottom: 1rem;
                }
                .clip-head {
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    gap: 1rem;
                    padding: .85rem 1.25rem;
                    border-bottom: 1px solid var(--line);
                    background: var(--surface-2);
                }
                .clip-head .name {
                    font-weight: 600;
                    font-size: .98rem;
                    color: var(--ink);
                    word-break: break-all;
                }
                .clip-head .tc {
                    font-family: ui-monospace, 'SF Mono', Menlo, Consolas, monospace;
                    font-size: .82rem;
                    color: var(--accent);
                    background: var(--accent-bg);
                    padding: .2rem .55rem;
                    border-radius: var(--radius-sm);
                    flex-shrink: 0;
                }

                .clip-body {
                    display: grid;
                    grid-template-columns: minmax(0, 1.3fr) minmax(0, 1fr);
                    gap: 1.25rem;
                    padding: 1.1rem 1.25rem;
                    align-items: start;          /* don't stretch columns to match */
                }
                @media (max-width: 820px) {
                    .clip-body { grid-template-columns: 1fr; }
                }

                /* Interactive thumbnail viewer (one big frame + picker strip).
                   Implemented with radio inputs so it works in offline HTML with no JS. */
                .viewer { display: flex; flex-direction: column; gap: .5rem; }
                .viewer input { position: absolute; opacity: 0; pointer-events: none; }
                .viewer .frames {
                    position: relative;
                    aspect-ratio: 16 / 9;
                    background: var(--surface-3);
                    border: 1px solid var(--line);
                    border-radius: var(--radius);
                    overflow: hidden;
                }
                .viewer .frames .frame {
                    position: absolute;
                    inset: 0;
                    width: 100%;
                    height: 100%;
                    object-fit: cover;
                    opacity: 0;
                    transition: opacity .18s ease-out;
                }
                .viewer .frames .frame.active { opacity: 1; }
                .viewer .frame-tc {
                    position: absolute;
                    left: .6rem; bottom: .6rem;
                    padding: .2rem .55rem;
                    border-radius: var(--radius-sm);
                    background: rgba(15,23,42,.72);
                    color: #fff;
                    font-family: ui-monospace, 'SF Mono', Menlo, Consolas, monospace;
                    font-size: .75rem;
                    letter-spacing: .02em;
                    opacity: 0;
                    transition: opacity .18s ease-out;
                }
                .viewer .frame-tc.active { opacity: 1; }
                .viewer .picks {
                    display: flex;
                    gap: .4rem;
                    overflow-x: auto;
                }
                .viewer .pick {
                    flex: 1 1 0;
                    min-width: 0;
                    cursor: pointer;
                    border: 2px solid transparent;
                    border-radius: var(--radius);
                    overflow: hidden;
                    background: var(--surface-3);
                    padding: 0;
                    aspect-ratio: 16 / 9;
                    position: relative;
                    transition: border-color .15s, transform .15s;
                }
                .viewer .pick:hover { transform: translateY(-1px); }
                .viewer .pick img { width: 100%; height: 100%; object-fit: cover; display: block; opacity: .75; transition: opacity .15s; }
                .viewer .pick.active { border-color: var(--accent); }
                .viewer .pick.active img { opacity: 1; }
                .viewer .pick .pick-tc {
                    position: absolute; inset: auto 0 0 0;
                    background: linear-gradient(transparent, rgba(0,0,0,.7));
                    color: #fff;
                    font-family: ui-monospace, 'SF Mono', Menlo, Consolas, monospace;
                    font-size: .65rem;
                    padding: 1.2rem .25rem .2rem;
                    text-align: center;
                }

                .thumbs-empty {
                    padding: 2rem 1rem;
                    background: var(--surface-3);
                    border: 1px dashed var(--line-2);
                    border-radius: var(--radius);
                    color: var(--ink-muted);
                    font-size: .82rem;
                    text-align: center;
                }

                /* Metadata grid */
                .meta-grid {
                    display: grid;
                    grid-template-columns: repeat(2, minmax(0, 1fr));
                    gap: .55rem 1.25rem;
                    align-content: start;
                }
                .meta-row {
                    display: grid;
                    grid-template-columns: 105px 1fr;
                    gap: .5rem;
                    padding: .35rem 0;
                    border-bottom: 1px solid var(--line);
                    align-items: baseline;
                }
                .meta-row:last-child { border-bottom: 0; }
                .meta-row .k {
                    font-size: .7rem;
                    letter-spacing: .1em;
                    text-transform: uppercase;
                    color: var(--ink-muted);
                    font-weight: 600;
                    display: inline-flex;
                    align-items: center;
                    gap: .4em;
                }
                .meta-row .k .meta-icon {
                    flex: none;
                    opacity: .75;
                }
                .meta-row .v {
                    font-size: .9rem;
                    color: var(--ink);
                    font-variant-numeric: tabular-nums;
                    word-break: break-word;
                }

                /* Footer */
                .footer {
                    text-align: center;
                    padding: 1.5rem;
                    color: var(--ink-muted);
                    font-size: .78rem;
                    border-top: 1px solid var(--line);
                    background: var(--surface);
                    margin-top: 2rem;
                }

                /* Print: switch to a light palette regardless of user theme so
                   ink rendering is predictable. All panels lose shadows/borders
                   that don't print well. */
                @page { margin: 14mm 14mm 16mm; size: A4; }
                @media print {
                    :root {
                        --bg: #ffffff; --surface: #ffffff; --surface-2: #fafafa; --surface-3: #f3f4f6;
                        --ink: #0f172a; --ink-2: #334155; --ink-3: #64748b; --ink-muted: #6b7280;
                        --line: #e5e7eb; --line-2: #d1d5db;
                        --accent: #2563eb; --accent-2: #1d4ed8; --accent-bg: #eff6ff;
                    }
                    html { font-size: 11pt; }
                    .clip-card { box-shadow: none; page-break-inside: avoid; break-inside: avoid; }
                    .reel-section { break-inside: auto; }
                    .toc { display: none; }
                    /* In print, the interactive viewer collapses to all 3 stills side-by-side. */
                    .viewer .picks { display: none; }
                    .viewer .frames { aspect-ratio: auto; height: auto; display: grid; grid-template-columns: repeat(3, 1fr); gap: .4rem; border: 0; background: transparent; overflow: visible; position: static; }
                    .viewer .frames .frame { position: static; opacity: 1 !important; aspect-ratio: 16 / 9; border: 1px solid var(--line); border-radius: var(--radius); }
                    .viewer .frame-tc { display: none; }
                }
            </style>
        """;

    public static string Project => """
            <style>
                .toc {
                    background: var(--surface);
                    border-bottom: 1px solid var(--line);
                    padding: 1rem 2rem;
                }
                .toc-inner { max-width: 1200px; margin: 0 auto; }
                .toc h2 {
                    font-size: .68rem;
                    color: var(--ink-muted);
                    text-transform: uppercase;
                    letter-spacing: .16em;
                    margin-bottom: .65rem;
                    font-weight: 600;
                }
                .toc-list { display: flex; gap: .5rem; flex-wrap: wrap; }
                .toc-chip {
                    background: var(--surface-3);
                    padding: .4rem .8rem;
                    border-radius: 999px;
                    color: var(--ink);
                    text-decoration: none;
                    display: inline-flex;
                    align-items: center;
                    gap: .5rem;
                    font-size: .8rem;
                    border: 1px solid var(--line);
                    transition: all .15s ease;
                }
                .toc-chip:hover {
                    background: var(--accent);
                    color: #fff;
                    border-color: var(--accent);
                    text-decoration: none;
                }
                .toc-chip:hover .count { color: rgba(255,255,255,.9); }
                .toc-chip .count {
                    color: var(--ink-muted);
                    font-size: .7rem;
                    font-weight: 500;
                }

                .reel-section { margin-bottom: 2.25rem; }
                .reel-head {
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    gap: 1rem;
                    padding: .85rem 0;
                    margin-bottom: .85rem;
                    border-bottom: 1px solid var(--line);
                }
                .reel-head h2 {
                    font-size: 1.15rem;
                    font-weight: 600;
                    display: flex;
                    align-items: center;
                    gap: .7rem;
                }
                .reel-head .reel-id {
                    background: var(--ink);
                    color: var(--surface);
                    padding: .22rem .65rem;
                    border-radius: var(--radius-sm);
                    font-size: .78rem;
                    font-weight: 600;
                    font-variant-numeric: tabular-nums;
                    letter-spacing: .02em;
                }
                .reel-head .cam { color: var(--ink-3); font-weight: 500; font-size: .92rem; }
                .reel-head-stats { display: flex; gap: 1.5rem; }
                .reel-head-stats .item {
                    display: flex;
                    flex-direction: column;
                    text-align: right;
                }
                .reel-head-stats .item .label {
                    font-size: .68rem;
                    color: var(--ink-muted);
                    text-transform: uppercase;
                    letter-spacing: .12em;
                    font-weight: 600;
                }
                .reel-head-stats .item .value {
                    font-size: .92rem;
                    font-weight: 500;
                    font-variant-numeric: tabular-nums;
                }

                @media print {
                    .reel-section { page-break-inside: auto; }
                    .reel-head { page-break-after: avoid; break-after: avoid; }
                }
            </style>
        """;

    /// <summary>CSS custom-property values per theme — the rest of the stylesheet is theme-invariant.</summary>
    private static string Palette(ReportTheme theme) => theme switch
    {
        ReportTheme.Dark => """
                    --bg:        #0d1117;
                    --surface:   #161b22;
                    --surface-2: #1c222b;
                    --surface-3: #21262d;
                    --ink:       #e6edf3;
                    --ink-2:     #b4bfca;
                    --ink-3:     #8b949e;
                    --ink-muted: #6e7681;
                    --line:      #30363d;
                    --line-2:    #3a4149;
                    --accent:    #58a6ff;
                    --accent-2:  #79b8ff;
                    --accent-bg: #1f3552;
            """,
        _ => """
                    --bg:        #f7f7f8;
                    --surface:   #ffffff;
                    --surface-2: #fafafa;
                    --surface-3: #f3f4f6;
                    --ink:       #0f172a;
                    --ink-2:     #334155;
                    --ink-3:     #64748b;
                    --ink-muted: #94a3b8;
                    --line:      #e5e7eb;
                    --line-2:    #d1d5db;
                    --accent:    #2563eb;
                    --accent-2:  #1d4ed8;
                    --accent-bg: #eff6ff;
            """,
    };
}
