// Luna camera report. Data arrives as /data.json (see src/view.rs); every string
// in it is printed literally, never read as markup.
//
// A4 landscape. Light is for paper, dark for screens on set. One accent: the
// tungsten of camera tape, which marks each camera the way an assistant labels
// the body. Coral is only for problems.

#let d = json("/data.json")
#let dark = d.theme == "dark"

#let paper = if dark { rgb("#14161a") } else { white }
#let ink = if dark { rgb("#eceff3") } else { rgb("#17191c") }
#let graphite = if dark { rgb("#a3abb6") } else { rgb("#5b626c") }
#let faint = if dark { rgb("#6f7783") } else { rgb("#a2a8b0") }
#let hairline = if dark { rgb("#2a2f37") } else { rgb("#dadde2") }
#let band = if dark { rgb("#1d2127") } else { rgb("#f1f2f4") }
#let tape = rgb("#e9a640")
#let tape-ink = rgb("#241703")
#let tungsten = if dark { rgb("#f2b75e") } else { rgb("#94590a") }
#let problem = if dark { rgb("#f2806f") } else { rgb("#b3402e") }

// Plex first; the serif only for scripts Plex doesn't cover (Hebrew).
#let sans = ("IBM Plex Sans", "Libertinus Serif")
#let condensed = ("IBM Plex Sans Condensed", "IBM Plex Sans", "Libertinus Serif")

#let thumb-width = 48mm
#let thumb-height = thumb-width * 9 / 16
#let gap = 1.2mm

// Camera-original names rarely contain a space; let them break after the
// separators cameras use instead.
#let breakable(name) = name.replace("_", "_\u{200b}").replace("-", "-\u{200b}").replace(".", ".\u{200b}")

#set document(title: if d.subtitle != none { d.title + " – " + d.subtitle } else { d.title }, author: if d.header.author != none { d.header.author } else { () })
#set text(font: sans, size: 7.6pt, fill: ink, lang: "en", hyphenate: false)
#set par(leading: 0.42em, spacing: 0.6em)

// The reel at the top of a page, for the running header: one that opens the
// page, else the one continuing from earlier pages.
#let reel-on(page) = {
  let reels = query(heading.where(level: 2))
  let opening = reels.filter(r => {
    let p = r.location().position()
    p.page == page and p.y < 40mm
  })
  if opening.len() > 0 { return opening.first() }
  let before = reels.filter(r => r.location().page() < page)
  if before.len() > 0 { before.last() } else { none }
}
// The camera that reel belongs to: the last camera heading before it.
#let camera-of(reel) = {
  let cams = query(heading.where(level: 1).before(reel.location()))
  if cams.len() > 0 { cams.last() } else { none }
}

#set page(
  paper: "a4",
  flipped: true,
  fill: paper,
  margin: (x: 12mm, top: 15mm, bottom: 13mm),
  header: context {
    let page = here().page()
    if page > 1 {
      set text(size: 7pt, fill: graphite)
      block(width: 100%, stroke: (bottom: 0.5pt + hairline), inset: (bottom: 4pt), grid(
        columns: (1fr, auto),
        text(weight: "medium", fill: ink, d.title),
        {
          let reel = reel-on(page)
          let cam = if reel != none { camera-of(reel) } else { none }
          if cam != none { cam.body }
          if cam != none and reel != none { h(8pt) }
          if reel != none { reel.body }
        },
      ))
    }
  },
  footer: context {
    set text(size: 6.8pt, fill: graphite)
    block(width: 100%, stroke: (top: 0.5pt + hairline), inset: (top: 4pt), grid(
      columns: (auto, 1fr, auto),
      column-gutter: 12pt,
      [Luna #d.luna_version#h(8pt)Generated #d.generated_at],
      align(center, text(fill: faint, d.source_root)),
      [Page #counter(page).display() of #counter(page).final().first()],
    ))
  },
)

// Cameras: a strip of camera tape carrying the name, then the camera's counts.
// The counts reach the heading through `section` so headings stay bookmarks.
#let section = state("section", "")
#show heading.where(level: 1): it => block(sticky: true, above: 14pt, below: 7pt, grid(
  columns: (auto, 1fr),
  column-gutter: 10pt,
  align: horizon,
  box(
    fill: tape,
    inset: (x: 8pt, y: 4.5pt),
    radius: 1pt,
    text(font: condensed, weight: "semibold", size: 12pt, fill: tape-ink, it.body),
  ),
  text(size: 8pt, weight: "regular", fill: graphite, context section.get()),
))

// Reels: a quiet band across the page.
#show heading.where(level: 2): it => block(sticky: true, above: 8pt, below: 0pt, block(
  width: 100%,
  fill: band,
  inset: (x: 7pt, y: 5pt),
  grid(
    columns: (1fr, auto),
    text(weight: "semibold", size: 8.6pt, it.body),
    text(weight: "regular", fill: graphite, context section.get()),
  ),
))

// End matter: Not included, Unreadable.
#show heading.where(level: 3): it => block(sticky: true, above: 16pt, below: 6pt, {
  text(weight: "semibold", size: 10pt, it.body)
  v(-2pt)
  line(length: 100%, stroke: 0.5pt + hairline)
})

// ---------------------------------------------------------------- title block

#grid(
  columns: (1fr, auto),
  column-gutter: 16pt,
  {
    text(weight: "semibold", size: 21pt, d.title)
    if d.subtitle != none {
      v(-6pt)
      text(size: 11pt, fill: graphite, d.subtitle)
    }
    let details = ()
    if d.header.company != none { details.push(("Company", d.header.company)) }
    if d.header.author != none { details.push(("Report by", d.header.author)) }
    for f in d.header.custom { details.push((f.label, f.value)) }
    if details.len() > 0 {
      v(4pt)
      grid(
        columns: details.len() * 2,
        column-gutter: (4pt, 16pt) * details.len(),
        ..details.map(((label, value)) => (text(fill: graphite, label), text(weight: "medium", value))).flatten(),
      )
    }
  },
  if d.header.logo != none { align(right + top, image(d.header.logo, height: 15mm)) },
)

#v(8pt)

#let stat(value, label, tone: ink) = {
  text(weight: "semibold", size: 12pt, fill: tone, value)
  linebreak()
  text(size: 6.8pt, fill: graphite, label)
}
#let stats = (
  stat(str(d.totals.cameras), if d.totals.cameras == 1 { "Camera" } else { "Cameras" }),
  stat(str(d.totals.cards), if d.totals.cards == 1 { "Card" } else { "Cards" }),
  stat(str(d.totals.clips), if d.totals.clips == 1 { "Clip" } else { "Clips" }),
  stat(d.total_size, "Data"),
)
#if d.show_problems {
  stats.push(stat(
    str(d.totals.clips_with_problems),
    "Need attention",
    tone: if d.totals.clips_with_problems > 0 { problem } else { ink },
  ))
}
#block(
  width: 100%,
  stroke: (top: 0.8pt + ink, bottom: 0.5pt + hairline),
  inset: (y: 6pt),
  grid(
    columns: (auto,) * stats.len() + (1fr,),
    column-gutter: 22pt,
    ..stats,
    align(right, {
      text(weight: "medium", d.source_root)
      linebreak()
      text(size: 6.8pt, fill: graphite)[Source]
    }),
  ),
)

#if d.cameras.len() == 0 [
  #v(1em)
  No camera clips in this folder.
]

// ---------------------------------------------------------------- clips

#let thumb(t) = {
  if t.src != none {
    box(width: thumb-width, height: thumb-height, fill: black, clip: true,
      image(t.src, width: 100%, height: 100%, fit: "contain"))
  } else {
    box(width: thumb-width, height: thumb-height, fill: band,
      align(center + horizon, text(size: 6.8pt, fill: faint)[No image]))
  }
}

#let cell-value(c) = {
  if c.missing { text(fill: faint, c.value) }
  else if c.accent { text(weight: "semibold", fill: tungsten, c.value) }
  else { c.value }
}

#let group(cells) = grid(
  columns: (auto, 1fr),
  column-gutter: 6pt,
  row-gutter: 3.4pt,
  ..cells.map(c => (text(size: 6.8pt, fill: graphite, c.label), cell-value(c))).flatten(),
)

#let clip-row(clip) = block(
  breakable: false,
  width: 100%,
  above: 0pt,
  below: 0pt,
  inset: (left: 6pt, y: 7pt),
  stroke: (
    bottom: 0.5pt + hairline,
    left: if clip.problem { 2pt + problem } else { none },
  ),
  grid(
    columns: (auto, 1fr),
    column-gutter: 10pt,
    grid(columns: clip.thumbs.len(), column-gutter: gap, ..clip.thumbs.map(thumb)),
    {
      grid(
        columns: (1fr, auto),
        column-gutter: 8pt,
        text(font: condensed, weight: "semibold", size: 9.4pt, breakable(clip.name)),
        if clip.recorded != none { text(fill: graphite, clip.recorded) },
      )
      v(4pt)
      // Camera and lens names are the long ones; timecodes and exposure are short.
      grid(
        columns: (34mm, 1fr, 38mm),
        column-gutter: 12pt,
        ..clip.groups.map(group),
      )
      if clip.identity != none {
        v(3pt)
        text(size: 6.5pt, fill: faint, clip.identity)
      }
      if clip.warnings.len() > 0 { v(3pt) }
      for (i, w) in clip.warnings.enumerate() {
        if i > 0 { linebreak() }
        text(size: 7.4pt, weight: "medium", fill: problem, w)
      }
    },
  ),
)

#for camera in d.cameras {
  section.update(camera.summary)
  heading(level: 1)[Camera #camera.name]
  for card in camera.cards {
    section.update(card.summary)
    heading(level: 2)[Reel #card.reel]
    for clip in card.clips { clip-row(clip) }
  }
}

// ---------------------------------------------------------------- end matter

#if d.not_included.len() > 0 {
  heading(level: 3)[Other files]
  for p in d.not_included [ #p \ ]
}

#if d.unreadable.len() > 0 {
  heading(level: 3)[Unreadable]
  grid(
    columns: (auto, auto, 1fr),
    column-gutter: 12pt,
    row-gutter: 4pt,
    ..d.unreadable.map(u => (text(fill: graphite, u.kind), u.path, text(fill: problem, u.reason))).flatten(),
  )
}
