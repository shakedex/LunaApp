// Luna camera report. Data arrives as /data.json (see src/view.rs).
// Layout is deliberately plain: the visual design has its own spec.

#let d = json("/data.json")
#let muted = luma(90)
#let thumb-height = 2.4cm

#set document(title: "Camera report")
#set text(size: 9pt)
#set page(
  paper: "a4",
  flipped: true,
  margin: (x: 1.5cm, y: 1.5cm),
  footer: context [
    #set text(size: 7pt, fill: muted)
    Luna #d.luna_version · #d.generated_at
    #h(1fr)
    #counter(page).display("1 / 1", both: true)
  ],
)

// Header
#grid(
  columns: (1fr, auto),
  [
    #text(size: 16pt, weight: "bold")[Camera report]
    #if d.header.production != none [ #text(size: 16pt)[ · #d.header.production] ]
    #if d.header.company != none [ \ #d.header.company ]
    #if d.header.author != none [ \ Report by #d.header.author ]
    #for f in d.header.custom [ \ #f.label: #f.value ]
  ],
  if d.header.logo != none { image(d.header.logo, height: 1.4cm) },
)
#text(fill: muted)[
  Source: #d.source_root \
  #d.totals.cameras cameras · #d.totals.cards cards · #d.totals.clips clips · #d.total_size · #d.totals.clips_with_problems clips with problems
]

#if d.cameras.len() == 0 [
  #v(1em)
  No camera clips found in this source.
]

#let thumb(t) = {
  if t.src != none {
    box(width: 100%, height: thumb-height, fill: black, image(t.src, width: 100%, height: thumb-height, fit: "contain"))
  } else {
    box(width: 100%, height: thumb-height, stroke: 0.5pt + muted, inset: 4pt,
      align(center + horizon, text(size: 7pt, fill: muted, t.reason)))
  }
}

#let clip-block(clip) = block(breakable: false, width: 100%, above: 0.8em, {
  line(length: 100%, stroke: 0.3pt + muted)
  // Camera-original names rarely contain a break point, so the file name gets
  // its own full-width line instead of a grid cell it would overflow.
  text(weight: "bold", clip.values.at(0))
  v(2pt)
  grid(columns: (1fr,) * 3, column-gutter: 4pt, ..clip.thumbs.map(thumb))
  v(3pt)
  grid(
    columns: (auto, 1fr) * 6,
    column-gutter: 5pt,
    row-gutter: 3pt,
    ..clip.values.enumerate().slice(1).map(((i, value)) => (
      text(fill: muted, d.labels.at(i)),
      value,
    )).flatten(),
  )
  v(2pt)
  text(size: 7pt, fill: muted, clip.identity)
  for w in clip.warnings [ \ #text(fill: red.darken(30%), w) ]
})

#for camera in d.cameras [
  #heading(level: 1)[Camera #camera.name]
  #for card in camera.cards [
    #heading(level: 2)[Reel #card.reel]
    #for clip in card.clips { clip-block(clip) }
  ]
]

#if d.not_included.len() > 0 [
  #heading(level: 1)[Not included]
  #for p in d.not_included [ - #p \ ]
]

#if d.unreadable.len() > 0 [
  #heading(level: 1)[Unreadable]
  #for u in d.unreadable [ - #u.kind #u.path: #u.reason \ ]
]
