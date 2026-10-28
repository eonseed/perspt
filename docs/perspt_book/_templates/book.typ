#import "cover-art.typ": cover-art

#let book(title: "Perspt", author: "", version: "", date: "", lang: "en", paper: "iso-b5",
  logo: none, copyright: "", accent: rgb("#059669"), numbering: false,
  version-label: "Version", body) = {
  let paper = "iso-b5"
  let obsidian = rgb("#090d16")
  let ink = rgb("#0f172a")
  let quiet = rgb("#475569")
  let rule = rgb("#cbd5e1")
  let emerald = rgb("#059669")
  let emerald-mint = rgb("#6ee7b7")
  let cyan = rgb("#0ea5e9")
  let amber = rgb("#d97706")
  let gold = rgb("#f59e0b")
  let sans = ("Liberation Sans", "DejaVu Sans")
  let mono = ("DejaVu Sans Mono", "Liberation Mono")

  set document(title: title, author: author)
  set text(font: ("Liberation Serif", "DejaVu Serif"), size: 10.2pt,
    fill: ink, lang: lang, hyphenate: true)
  set par(justify: true, leading: .64em, spacing: .8em, first-line-indent: 0em)
  set heading(numbering: "1.1")
  show heading: set text(font: sans, hyphenate: false)
  show heading.where(level: 1): it => {
    if it.outlined { pagebreak(weak: true) }
    block(above: 10mm, below: 8mm, sticky: true)[
      #grid(
        columns: (14mm, 8mm, 1fr),
        column-gutter: 1.5mm,
        rect(width: 14mm, height: 2.6pt, fill: emerald, radius: 1pt),
        rect(width: 8mm, height: 2.6pt, fill: amber, radius: 1pt),
        [],
      )
      #v(4mm)
      #text(size: 21pt, weight: "bold", fill: obsidian, it.body)
    ]
  }
  show heading.where(level: 2): it => block(above: 1.6em, below: .65em, sticky: true)[
    #text(size: 13.5pt, weight: "bold", fill: ink, it.body)
    #v(1.5mm)
    #line(length: 100%, stroke: 0.5pt + rule)
  ]
  show heading.where(level: 3): it => block(above: 1.25em, below: .5em, sticky: true,
    text(size: 11pt, weight: "bold", fill: emerald, it.body))
  show link: set text(fill: emerald)
  show raw: set text(font: mono, size: .79em, hyphenate: false)
  show raw.where(block: true): it => block(width: 100%, breakable: true,
    inset: 9.5pt, fill: rgb("#f8fafc"), stroke: (top: 1.8pt + emerald, rest: 0.4pt + rule), radius: 4pt, it)
  set table(stroke: .4pt + rule, inset: 6pt,
    fill: (_, y) => if y == 0 { rgb("#ecfdf5") } else { none })
  show table: set text(size: .86em)
  show table: set par(justify: false)
  show figure.caption: set text(font: sans, size: 8.8pt, fill: quiet)
  set figure(gap: 3mm)

  // Architectural Dark-Header Cover Page (Distinct to Perspt)
  page(paper: paper, margin: 0pt, header: none, footer: none, numbering: none)[
    #set par(justify: false, leading: .35em, first-line-indent: 0pt, spacing: 0pt)
    // Top Obsidian Command Deck Band
    #block(width: 100%, fill: obsidian, inset: (x: 18mm, top: 15mm, bottom: 13mm))[
      #grid(
        columns: (auto, 1fr, auto),
        column-gutter: 3.5mm,
        align: horizon,
        if logo != none { image(logo, height: 9.5mm) },
        text(font: mono, size: 7.8pt, weight: "bold", tracking: .16em, fill: emerald-mint)[
          PERSPT :: DEVELOPER HANDBOOK
        ],
        rect(
          fill: rgb("#111827"),
          stroke: 0.7pt + emerald,
          radius: 3pt,
          inset: (x: 5pt, y: 3pt),
          text(font: mono, size: 7pt, weight: "bold", fill: gold)[v#version],
        ),
      )
      #v(9mm)
      #text(font: sans, size: 21pt, weight: "bold", tracking: -.02em, hyphenate: false, fill: rgb("#f8fafc"))[
        Build with AI in your terminal. \
        #text(fill: emerald-mint)[Ship code verified by your compiler.]
      ]
      #v(5mm)
      #text(font: sans, size: 10.5pt, fill: rgb("#cbd5e1"))[
        The complete guide to Perspt's command-line pipes, interactive TUI, \
        compiler-verified SRBN coding agent, and embeddable Rust SDK.
      ]
    ]
    // Spectrum Divider Bar (Emerald -> Cyan -> Gold)
    #grid(
      columns: (1fr, 1fr, 1fr),
      rect(width: 100%, height: 3.5pt, fill: emerald),
      rect(width: 100%, height: 3.5pt, fill: cyan),
      rect(width: 100%, height: 3.5pt, fill: gold),
    )
    // Lower Cover Body
    #block(width: 100%, inset: (x: 18mm, top: 8mm, bottom: 14mm))[
      #align(center, cover-art())
      #v(7mm)
      #grid(
        columns: (1fr, 1fr),
        gutter: 3mm,
        rect(width: 100%, fill: rgb("#f0fdf4"), stroke: 0.5pt + rgb("#a7f3d0"), radius: 4pt, inset: 6pt)[
          #text(font: mono, size: 6.8pt, weight: "bold", fill: emerald)[01 · SIMPLE CLI & TUI]\
          #v(1.5mm)
          #text(font: sans, size: 8pt, fill: quiet)[Pipe-ready `simple-chat` and full-screen `perspt chat` across 9+ providers.]
        ],
        rect(width: 100%, fill: rgb("#fffbeb"), stroke: 0.5pt + rgb("#fde68a"), radius: 4pt, inset: 6pt)[
          #text(font: mono, size: 6.8pt, weight: "bold", fill: amber)[02 · SRBN AGENT & RUST SDK]\
          #v(1.5mm)
          #text(font: sans, size: 8pt, fill: quiet)[Compiler-verified `perspt agent` and reusable `perspt-sdk` crates.]
        ],
      )
      #v(7mm)
      #line(length: 100%, stroke: .6pt + obsidian)
      #v(3.5mm)
      #grid(
        columns: (1fr, auto),
        align: horizon,
        text(font: sans, size: 10pt, weight: "bold", fill: obsidian, author),
        text(font: mono, size: 7.5pt, tracking: .1em, fill: quiet)[EDITION #version · HÓZHÓ],
      )
    ]
  ]

  set page(paper: paper, margin: (inside: 21mm, outside: 19mm, top: 22mm, bottom: 20mm),
    header: context align(end, text(font: mono, size: 7pt, tracking: 0.08em, fill: quiet)[PERSPT :: CLI · TUI · SRBN AGENT · SDK]),
    footer: context {
      line(length: 100%, stroke: .4pt + rule)
      v(2mm)
      text(font: mono, size: 7.5pt, fill: quiet)[#version-label #version]
      h(1fr)
      text(font: mono, size: 8.5pt, weight: "bold", fill: emerald, counter(page).display("1"))
    })
  counter(page).update(1)
  outline(title: [Contents], depth: 2, indent: 1.2em)
  pagebreak()
  body
}
