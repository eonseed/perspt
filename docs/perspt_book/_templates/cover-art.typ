// Commands shared by the landing page and the book cover.
#let cover-art() = block(
  width: 300pt,
  fill: rgb("#0c1724"),
  radius: 8pt,
  inset: 18pt,
)[
  #set text(font: ("Liberation Sans", "DejaVu Sans"), fill: rgb("#f1f5f9"), size: 10pt)
  #text(size: 8pt, weight: "bold", tracking: 0.12em, fill: rgb("#6ee7b7"))[PERSPT / AI IN THE TERMINAL]
  #v(12pt)
  #text(size: 24pt, weight: "bold")[Chat with AI.\ Get help with code.]
  #v(16pt)
  #for (label, command, desc, color) in (
    ("CHAT WITH AI", "perspt chat", "Ask questions and discuss code.", "#6ee7b7"),
    ("SIMPLE TERMINAL CHAT", "perspt simple-chat", "Type a message. Read the answer.", "#7dd3fc"),
    ("RUN A CODING TASK", "perspt agent \"Fix the failing tests\"", "Edit a project copy, build, test, and review.", "#fcd34d"),
    ("USE THE RUST LIBRARIES", "perspt-sdk + perspt-coding", "Add Perspt to a Rust application.", "#c4b5fd"),
  ) {
    block(width: 100%, stroke: (top: 0.5pt + rgb("#334155")), inset: (top: 9pt, bottom: 9pt))[
      #text(size: 7pt, weight: "bold", tracking: 0.07em, fill: rgb(color))[#label]
      #v(3pt)
      #text(font: ("DejaVu Sans Mono", "Liberation Mono"), size: 9pt, weight: "bold")[#command]
      #v(3pt)
      #text(size: 8.5pt, fill: rgb("#cbd5e1"))[#desc]
    ]
  }
]
