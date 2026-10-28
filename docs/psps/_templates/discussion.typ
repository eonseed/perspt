#import "theme.typ": gd-palette, gd-serif, gd-sans, gd-mono, gd-script-fonts, configure-document
// Perspt Specification Proposal (PSP) Typst layout for Guidedog discussion records.
#let book(
  title: "", author: "", version: "", date: "", lang: "en", paper: "a4",
  numbering: false, version-label: "Version", logo: none, copyright: "",
  record: (id: "PSP", catalog: "", fields: ()),
  accent: gd-palette.accent,
  serif: gd-serif,
  sans: gd-sans,
  mono: gd-mono,
  size: 10.5pt,
  contents: true,
  contents-label: "Contents",
  memo: true,
  body,
) = {
  show: configure-document.with(lang: lang, serif: serif, sans: sans, mono: mono,
    size: size, accent: accent)
  sans = sans + gd-script-fonts(lang)
  let ink = gd-palette.ink
  let quiet = gd-palette.quiet
  let rule = gd-palette.rule
  let wash = gd-palette.paper
  let sans-text = text.with(font: sans, number-type: "lining")
  let small = sans-text.with(size: 8pt, fill: quiet)
  set document(title: title, author: if author == "" { () } else { author })
  set page(paper: paper, margin: (x: 24mm, top: 23mm, bottom: 23mm),
    numbering: "1", header-ascent: 30%, footer-descent: 35%,
    header: context {
      if here().page() > 1 {
        block(width: 100%, below: 4mm, {
          grid(columns: (auto, 1fr), column-gutter: 8mm,
            small(weight: "bold", fill: accent, record.id), align(end, small(record.catalog)))
          v(2mm)
          line(length: 100%, stroke: 0.5pt + rule)
        })
      }
    },
    footer: context {
      line(length: 100%, stroke: 0.4pt + rule)
      v(2mm)
      grid(columns: (1fr, auto), column-gutter: 8mm,
        small(record.id),
        small[#counter(page).display() / #counter(page).final().first()])
    })

  let state-chip(value) = {
    let tone = (final: gd-palette.green, active: gd-palette.accent, accepted: gd-palette.green,
      discussion: gd-palette.blue, committed: gd-palette.violet,
      abandoned: quiet, withdrawn: quiet, rejected: quiet,
      draft: gd-palette.amber, prediscussion: gd-palette.amber).at(value, default: accent)
    box(inset: (x: 0.65em, y: 0.25em), radius: 4pt, fill: tone.lighten(90%),
      stroke: 0.6pt + tone.lighten(40%),
      sans-text(size: 8.5pt, fill: tone.darken(15%), weight: "bold", upper(value)))
  }
  let field(pair) = block(breakable: true, {
    small(size: 7.4pt, weight: "bold", tracking: 0.06em, upper(pair.first()))
    v(1mm)
    let value = pair.last()
    let shown = if pair.first() in ("State", "Status") { state-chip(value) }
      else if value.starts-with("https://") or value.starts-with("http://") { link(value, value) }
      else { value }
    sans-text(size: 9pt, fill: ink, shown)
  })
  let front-matter(title) = {
    set text(weight: "regular")
    set par(justify: false)
    block(width: 100%, inset: 13pt, radius: 5pt, stroke: (top: 2pt + accent, rest: 0.6pt + rule),
      fill: wash, breakable: true, {
        grid(columns: (1fr, auto), column-gutter: 8mm,
          sans-text(size: 9.5pt, weight: "bold", tracking: 0.06em, fill: accent, upper(record.catalog)),
          sans-text(size: 11pt, weight: "bold", fill: accent, record.id))
        v(4mm)
        sans-text(size: 21pt, weight: "bold", tracking: -0.015em,
          par(leading: 0.3em, title))
        v(3mm)
        line(length: 100%, stroke: 0.5pt + rule)
        v(3mm)
        let compact = record.fields.filter(pair => pair.first() not in ("Discussion", "Labels"))
        grid(columns: (1fr, 1fr), column-gutter: 8mm, row-gutter: 2.5mm,
          ..compact.map(field))
        for pair in record.fields.filter(pair => pair.first() in ("Discussion", "Labels")) {
          v(2.5mm)
          field(pair)
        }
      })
    if memo {
      block(width: 100%, above: 4mm, below: 4mm, inset: 10pt, radius: 3pt,
        stroke: (left: 2pt + accent), fill: gd-palette.soft, breakable: true, {
          sans-text(size: 9pt, weight: "bold", fill: accent, [Specification status])
          v(1.5mm)
          text(size: 9pt, [The Perspt Specification Proposal catalog defines the review process and lifecycle state machine.
            #if record.id.match(regex("X+$")) != none {
              [This placeholder identifies a provisional draft.]
            } else { [This numbered record forms part of the Perspt specification archive.] }])
        })
    }
    if contents and query(heading.where(level: 2)).len() > 0 {
      show outline.entry: set text(size: 8.5pt)
      set par(leading: 0.4em, spacing: 0.3em)
      block(above: 2mm, below: 2mm, sans-text(size: 10pt, weight: "bold", contents-label))
      outline(title: none, target: heading.where(level: 2), depth: 2, indent: 0pt)
      v(3mm)
    }
  }
  let section-number(..values) = if values.pos().len() > 1 {
    std.numbering("1.1.", ..values.pos().slice(1))
  }
  set heading(numbering: section-number)
  show heading.where(level: 1): set heading(numbering: none)
  show heading: set text(font: sans, fill: ink, hyphenate: false)
  show heading: set par(justify: false, first-line-indent: 0em)
  show heading.where(level: 1): it => context {
    if not it.outlined {
      return block(above: 1em, below: 0.5em, sticky: true,
        sans-text(size: 11pt, weight: "bold", it.body))
    }
    let first = query(heading.where(level: 1, outlined: true)).first().location() == it.location()
    if first {
      front-matter(it.body)
    } else {
      block(above: 1.5em, below: 0.6em, sticky: true,
        sans-text(size: 14pt, weight: "bold", it.body))
    }
  }
  show heading: it => if it.level > 1 {
    context {
      let counters = counter(heading).at(it.location()).slice(1)
      let number = std.numbering("1.1.", ..counters)
      block(above: if it.level == 2 { 1.7em } else { 1.1em }, below: 0.6em, sticky: true,
        sans-text(size: if it.level == 2 { 11.5pt } else { 10.5pt }, weight: "bold", {
          text(fill: accent, number)
          h(0.65em)
          it.body
        }))
    }
  } else { it }

  show raw: set text(font: mono, hyphenate: false, ligatures: false)
  show raw.where(block: false): set text(size: 0.88em)
  show raw.where(block: true): it => block(width: 100%, breakable: true,
    fill: wash, stroke: (left: 2pt + accent.lighten(25%)), inset: 9pt,
    text(size: 0.82em, it))
  show figure.where(kind: raw): set block(breakable: true)
  show figure.where(kind: table): set block(breakable: true)
  show figure.caption: set text(font: sans, size: 0.86em, fill: quiet)
  show table: set text(size: 0.90em)
  set table(inset: (x: 6pt, y: 5pt), stroke: 0.4pt + rule,
    fill: (x, y) => if y == 0 { gd-palette.soft } else { none })
  set list(indent: 0.6em, body-indent: 0.6em)
  set enum(indent: 0.4em, body-indent: 0.6em)
  show link: set text(fill: accent)
  show quote.where(block: true): set block(inset: (left: 12pt),
    stroke: (left: 1pt + rule), breakable: true)
  set footnote.entry(separator: line(length: 22%, stroke: 0.4pt + rule), gap: 0.5em)
  show footnote.entry: set text(size: 0.83em)

  body
}
