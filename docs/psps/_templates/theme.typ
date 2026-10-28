// Perspt shared Typst PDF theme for Guidedog discussion records.
#let gd-palette = (
  accent: rgb("#0f766e"), ink: rgb("#0f172a"), quiet: rgb("#475569"),
  rule: rgb("#cbd5e1"), paper: rgb("#f8fafc"), soft: rgb("#f0fdfa"),
  blue: rgb("#0284c7"), green: rgb("#059669"), violet: rgb("#7c3aed"),
  amber: rgb("#d97706"),
)
#let gd-serif = ("Liberation Serif", "DejaVu Serif")
#let gd-sans = ("Liberation Sans", "DejaVu Sans")
#let gd-mono = ("DejaVu Sans Mono", "Liberation Mono")
#let gd-language(lang) = {
  let parts = lang.replace("_", "-").split("-")
  let code = lower(parts.first())
  if code.len() < 2 or code.len() > 3 or code.match(regex("^[a-z]+$")) == none { code = "en" }
  let region = if parts.len() > 1 and parts.at(1).match(regex("^[A-Za-z]{2}$")) != none {
    upper(parts.at(1))
  } else { none }
  (code: code, region: region)
}
#let gd-script-fonts(lang) = {
  let language = gd-language(lang)
  let cjk = if language.code == "zh" and language.region in ("TW", "HK") {
    "Noto Serif CJK TC"
  } else {
    (zh: "Noto Serif CJK SC", ja: "Noto Serif CJK JP",
      ko: "Noto Serif CJK KR", hi: "Noto Serif Devanagari").at(language.code, default: none)
  }
  if cjk == none { () } else { (cjk,) }
}
#let configure-document(body, lang: "en", serif: gd-serif, sans: gd-sans,
  mono: gd-mono, size: 10.5pt, accent: gd-palette.accent) = {
  let language = gd-language(lang)
  let scripts = gd-script-fonts(lang)
  state("gd-math-sans").update(_ => sans + scripts)
  set text(font: serif + scripts, size: size, fill: gd-palette.ink,
    lang: language.code, region: language.region, hyphenate: true, number-type: "lining",
    costs: (hyphenation: 100%, runt: 180%, widow: 100%, orphan: 100%))
  set par(justify: true, linebreaks: "optimized", leading: 0.62em, spacing: 0.65em,
    first-line-indent: 0em)
  set block(spacing: 0.86em)
  show heading: set text(font: sans + scripts, hyphenate: false)
  show heading: set par(justify: false, first-line-indent: 0em)
  show raw: set text(font: mono, hyphenate: false, ligatures: false)
  show link: set text(fill: accent)
  body
}
