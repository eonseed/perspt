// Creative cover & hero illustration for Perspt:
// "The Terminal Spectrum & SRBN Verification Prism" — showing Simple CLI, Terminal UI,
// and the SRBN Coding Agent converging through compiler/LSP/test rings to V(x) = 0.

#let obsidian = rgb("#090d16")
#let slate-card = rgb("#111827")
#let slate-line = rgb("#1e293b")
#let emerald-deep = rgb("#064e3b")
#let emerald = rgb("#059669")
#let emerald-glow = rgb("#10b981")
#let emerald-mint = rgb("#6ee7b7")
#let cyan = rgb("#0ea5e9")
#let cyan-soft = rgb("#7dd3fc")
#let amber = rgb("#d97706")
#let gold = rgb("#f59e0b")
#let coral = rgb("#f43f5e")
#let paper = rgb("#f8fafc")
#let muted = rgb("#94a3b8")

#let cover-illustration() = box(
  width: 268pt,
  height: 224pt,
  fill: obsidian,
  radius: 10pt,
  stroke: 1.2pt + emerald,
  clip: true,
)[
  // Subtle terminal titlebar at top of the illustration card
  #place(
    top + left,
    rect(
      width: 268pt,
      height: 20pt,
      fill: slate-card,
      stroke: (bottom: 0.8pt + slate-line),
    )[
      #place(dx: 9pt, dy: 6.5pt, circle(radius: 3.2pt, fill: coral))
      #place(dx: 19pt, dy: 6.5pt, circle(radius: 3.2pt, fill: gold))
      #place(dx: 29pt, dy: 6.5pt, circle(radius: 3.2pt, fill: emerald-glow))
      #place(
        dx: 44pt,
        dy: 5.5pt,
        text(font: ("DejaVu Sans Mono", "Liberation Mono"), size: 6.6pt, weight: "bold", fill: emerald-mint)[
          \$\_ perspt :: CLI · TUI · SRBN-AGENT · SDK
        ],
      )
    ],
  )

  // Subtle coordinate grid lines inside the dark canvas
  #for x in (44pt, 88pt, 134pt, 180pt, 224pt) {
    place(
      top + left,
      line(start: (x, 20pt), end: (x, 224pt), stroke: 0.4pt + slate-line),
    )
  }
  #for y in (56pt, 94pt, 132pt, 170pt, 204pt) {
    place(
      top + left,
      line(start: (0pt, y), end: (268pt, y), stroke: 0.4pt + slate-line),
    )
  }

  // Three input mode badges at the top row (Simple CLI, Terminal UI, SRBN Agent)
  #for (dx, label, sub, col) in (
    (12pt, "01 SIMPLE CLI", "stdin | stdout", cyan-soft),
    (96pt, "02 TERMINAL UI", "ratatui + /mcp", emerald-mint),
    (180pt, "03 SRBN AGENT", "compile + test", gold),
  ) {
    place(
      dx: dx,
      dy: 28pt,
      rect(
        width: 76pt,
        height: 26pt,
        radius: 4pt,
        fill: slate-card,
        stroke: 0.9pt + col,
        inset: (x: 5pt, y: 3.5pt),
      )[
        #text(font: ("DejaVu Sans Mono", "Liberation Mono"), size: 5.8pt, weight: "bold", fill: col)[#label]\
        #text(font: ("DejaVu Sans Mono", "Liberation Mono"), size: 5.3pt, fill: paper)[#sub]
      ],
    )
  }

  // Concentric verification barrier rings around the central attractor (134pt, 136pt)
  #for (w, h, col, dash) in (
    (216pt, 102pt, slate-line, none),
    (176pt, 82pt, cyan.transparentize(30%), "dashed"),
    (132pt, 62pt, emerald-glow.transparentize(15%), none),
    (84pt, 38pt, gold, none),
  ) {
    place(
      dx: 134pt - w / 2,
      dy: 138pt - h / 2,
      ellipse(
        width: w,
        height: h,
        fill: col.transparentize(90%),
        stroke: if dash != none {
          (paint: col, thickness: 1.1pt, dash: dash)
        } else {
          (paint: col, thickness: 1.2pt)
        },
      ),
    )
  }

  // Converging trajectories from the 3 modes into the verified core V(x) = 0
  #for (pts, col) in (
    (((50pt, 54pt), (68pt, 88pt), (96pt, 118pt), (134pt, 138pt)), cyan-soft),
    (((134pt, 54pt), (134pt, 84pt), (134pt, 112pt), (134pt, 138pt)), emerald-mint),
    (((218pt, 54pt), (200pt, 88pt), (172pt, 118pt), (134pt, 138pt)), gold),
  ) {
    place(
      top + left,
      curve(
        stroke: (paint: col, thickness: 1.6pt),
        curve.move(pts.at(0)),
        ..pts.slice(1).map(p => curve.line(p)),
      ),
    )
    for p in pts.slice(1, 3) {
      place(
        dx: p.at(0) - 3pt,
        dy: p.at(1) - 3pt,
        circle(radius: 3pt, fill: obsidian, stroke: 1.1pt + col),
      )
    }
  }

  // Central Verified Core V(x) = 0 badge
  #place(
    dx: 134pt - 30pt,
    dy: 138pt - 11pt,
    rect(
      width: 60pt,
      height: 22pt,
      radius: 11pt,
      fill: emerald-deep,
      stroke: 1.4pt + emerald-mint,
    )[
      #align(center + horizon)[
        #text(font: ("DejaVu Sans Mono", "Liberation Mono"), size: 7.2pt, weight: "bold", fill: paper)[
          V(x) = 0 ✓
        ]
      ]
    ],
  )

  // Bottom SDK & Toolchain status strip
  #place(
    dx: 12pt,
    dy: 196pt,
    rect(
      width: 244pt,
      height: 19pt,
      radius: 4pt,
      fill: slate-card,
      stroke: 0.7pt + slate-line,
      inset: (x: 8pt, y: 4pt),
    )[
      #grid(
        columns: (1fr, auto),
        align: horizon,
        text(font: ("DejaVu Sans Mono", "Liberation Mono"), size: 5.8pt, fill: emerald-mint)[
          RUST SDK: perspt-sdk · perspt-coding · perspt-agent
        ],
        text(font: ("DejaVu Sans Mono", "Liberation Mono"), size: 5.8pt, weight: "bold", fill: gold)[
          PASS
        ],
      )
    ],
  )
]

#let cover-art = cover-illustration
