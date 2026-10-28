#import "cover-art.typ": cover-art

#let book(title: "Perspt", author: "", version: "", date: "", lang: "en", paper: "iso-b5",
  logo: none, copyright: "", accent: rgb("#0f766e"), numbering: false,
  version-label: "Version", body) = {
  let paper = "iso-b5"
  let ink = rgb("#0f172a")
  let quiet = rgb("#475569")
  let rule = rgb("#cbd5e1")
  let amber = rgb("#d97706")
  let sans = ("Liberation Sans", "DejaVu Sans")
  set document(title: title, author: author)
  set text(font: ("Liberation Serif", "DejaVu Serif"), size: 10.2pt,
    fill: ink, lang: lang, hyphenate: true)
  set par(justify: true, leading: .64em, spacing: .8em, first-line-indent: 0em)
  set heading(numbering: "1.1")
  show heading: set text(font: sans, hyphenate: false)
  show heading.where(level: 1): it => {
    if it.outlined { pagebreak(weak: true) }
    block(above: 10mm, below: 8mm, sticky: true)[
      #line(length: 20mm, stroke: 2.4pt + amber)
      #v(4.5mm)
      #text(size: 22pt, weight: "bold", fill: accent, it.body)
    ]
  }
  show heading.where(level: 2): it => block(above: 1.6em, below: .65em, sticky: true,
    text(size: 13.5pt, weight: "bold", fill: ink, it.body))
  show heading.where(level: 3): it => block(above: 1.25em, below: .5em, sticky: true,
    text(size: 11pt, weight: "bold", fill: accent, it.body))
  show link: set text(fill: accent)
  show raw: set text(font: ("DejaVu Sans Mono", "Liberation Mono"), size: .79em, hyphenate: false)
  show raw.where(block: true): it => block(width: 100%, breakable: true,
    inset: 9.5pt, fill: rgb("#f8fafc"), stroke: (left: 2pt + accent, rest: 0.4pt + rule), radius: (right: 3pt), it)
  set table(stroke: .4pt + rule, inset: 6pt,
    fill: (_, y) => if y == 0 { rgb("#f0fdfa") } else { none })
  show table: set text(size: .86em)
  show table: set par(justify: false)
  show figure.caption: set text(font: sans, size: 8.8pt, fill: quiet)
  set figure(gap: 3mm)
  page(paper: paper, margin: (x: 19mm, top: 16mm, bottom: 16mm),
    header: none, footer: none, numbering: none)[
    #set par(justify: false, leading: .3em, first-line-indent: 0pt, spacing: 0pt)
    #grid(columns: (auto, 1fr), column-gutter: 3.5mm, align: horizon,
      if logo != none { image(logo, height: 10mm) },
      text(font: sans, size: 8pt, weight: "bold", tracking: .22em, fill: accent)[THE PERSPT BOOK])
    #v(4mm)
    #line(length: 100%, stroke: .4pt + rule)
    #v(16mm)
    #block(text(font: sans, size: 32pt, weight: "bold", tracking: -.03em, fill: ink)[
      Generative velocity. \
      #text(fill: accent)[Deterministic stability.]
    ])
    #v(7mm)
    #line(length: 16mm, stroke: 2.2pt + amber)
    #v(5mm)
    #block(text(size: 12.5pt, style: "italic", fill: quiet)[
      Stabilized Recursive Barrier Networks, governed tool harnesses, \
      and high-performance AI engineering in Rust])
    #v(1fr)
    #align(center, cover-art())
    #v(1fr)
    #line(length: 100%, stroke: .6pt + ink)
    #v(4mm)
    #text(font: sans, size: 10.5pt, weight: "bold", author)
    #h(1fr)
    #text(font: sans, size: 8pt, tracking: .12em, fill: quiet)[EDITION #version / 2026]
  ]
  set page(paper: paper, margin: (inside: 21mm, outside: 19mm, top: 22mm, bottom: 20mm),
    header: context align(end, text(font: sans, size: 7.5pt, tracking: 0.08em, fill: quiet)[PERSPT / SRBN ARCHITECTURE & GUIDE]),
    footer: context {
      line(length: 100%, stroke: .4pt + rule)
      v(2mm)
      text(font: sans, size: 8pt, fill: quiet)[#version-label #version]
      h(1fr)
      text(font: sans, size: 9pt, weight: "bold", fill: accent, counter(page).display("1"))
    })
  counter(page).update(1)
  outline(title: [Contents], depth: 2, indent: 1.2em)
  pagebreak()
  body
}
