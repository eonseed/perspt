// Command-oriented artwork shared by the landing page and the book cover.
#let cover-art() = block(
  width: 300pt,
  fill: rgb("#0c1724"),
  radius: 8pt,
  inset: 18pt,
)[
  #set text(font: ("Liberation Sans", "DejaVu Sans"), fill: rgb("#f1f5f9"), size: 10pt)
  #text(size: 8pt, weight: "bold", tracking: 0.12em, fill: rgb("#6ee7b7"))[PERSPT / IN THE TERMINAL]
  #v(12pt)
  #text(size: 24pt, weight: "bold")[A question.\ A checked change.]
  #v(16pt)
  #for (label, command, desc, color) in (
    ("01 / CONVERSATION", "perspt chat", "Think through a problem with a model.", "#6ee7b7"),
    ("02 / PLAIN TERMINAL", "perspt simple-chat", "Use line editing and session logging.", "#7dd3fc"),
    ("03 / CODE CHANGE", "perspt agent -w ./app", "Propose, check, repair, then review.", "#fcd34d"),
    ("04 / RUST LIBRARIES", "perspt-sdk + perspt-coding", "Build a custom verification workflow.", "#c4b5fd"),
  ) {
    block(width: 100%, stroke: (top: 0.5pt + rgb("#334155")), inset: (top: 9pt, bottom: 9pt))[
      #text(size: 7pt, weight: "bold", tracking: 0.07em, fill: rgb(color))[#label]
      #v(3pt)
      #text(font: ("DejaVu Sans Mono", "Liberation Mono"), size: 10pt, weight: "bold")[#command]
      #v(3pt)
      #text(size: 8.5pt, fill: rgb("#cbd5e1"))[#desc]
    ]
  }
]
