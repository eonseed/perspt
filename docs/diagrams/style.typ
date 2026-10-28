// Shared visual language for Perspt Typst diagrams.
// High-contrast editorial typography + geometric precision.

#let ink = rgb("#0f172a")
#let ink-Mid = rgb("#334155")
#let muted = rgb("#64748b")
#let rule = rgb("#cbd5e1")
#let rule-light = rgb("#e2e8f0")
#let paper = rgb("#ffffff")
#let panel = rgb("#f8fafc")

// Accent palette: SRBN cyan/teal (stabilizer), amber/gold (energy/gate),
// crimson (barrier/hazard), emerald (verified commit), violet (kernel/sheaf).
#let teal-dark = rgb("#164e63")
#let teal = rgb("#0e7490")
#let teal-bright = rgb("#06b6d4")
#let teal-soft = rgb("#ecfeff")

#let amber-dark = rgb("#78350f")
#let amber = rgb("#b45309")
#let amber-bright = rgb("#f59e0b")
#let amber-soft = rgb("#fef3c7")

#let crimson-dark = rgb("#7f1d1d")
#let crimson = rgb("#b91c1c")
#let crimson-soft = rgb("#fef2f2")

#let emerald-dark = rgb("#064e3b")
#let emerald = rgb("#047857")
#let emerald-soft = rgb("#ecfdf5")

#let violet-dark = rgb("#3b0764")
#let violet = rgb("#6d28d9")
#let violet-soft = rgb("#f5f3ff")

#let diagram-page(width: 720pt, margin: 14pt, body) = {
  set page(width: width, height: auto, margin: margin, fill: paper)
  set text(font: ("Liberation Sans", "DejaVu Sans"), size: 9pt, fill: ink)
  set par(leading: 0.48em)
  body
}

#let badge(label, fg: teal-dark, bg: teal-soft, border: teal) = box(
  inset: (x: 5.5pt, y: 2.5pt),
  radius: 3pt,
  fill: bg,
  stroke: 0.65pt + border,
  text(size: 7.2pt, weight: "bold", tracking: 0.06em, fill: fg)[#upper(label)],
)

#let section-banner(kicker, title, subtitle: none) = block(
  width: 100%,
  inset: (bottom: 8pt),
  stroke: (bottom: 1.4pt + ink),
  [
    #text(size: 7.4pt, weight: "bold", tracking: 0.14em, fill: teal)[#upper(kicker)]
    #v(2pt)
    #text(size: 14pt, weight: "black", tracking: -0.02em, fill: ink)[#title]
    #if subtitle != none [
      #v(2pt)
      #text(size: 8.5pt, fill: muted)[#subtitle]
    ]
  ],
)

#let card(
  title,
  body,
  kicker: none,
  fill-color: panel,
  border-color: rule,
  accent-color: teal,
  width: 100%,
) = block(
  width: width,
  fill: fill-color,
  stroke: (top: 2.4pt + accent-color, rest: 0.75pt + border-color),
  radius: 4pt,
  inset: (x: 9pt, y: 7.5pt),
  [
    #if kicker != none [
      #text(size: 6.8pt, weight: "bold", tracking: 0.1em, fill: accent-color)[#upper(kicker)]
      #v(1.5pt)
    ]
    #text(size: 9.2pt, weight: "bold", fill: ink)[#title]
    #v(3pt)
    #text(size: 8pt, fill: ink-Mid)[#body]
  ],
)
