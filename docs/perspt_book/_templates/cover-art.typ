// Cover illustration for the Perspt Book:
// 3D Lyapunov Attractor Funnel V(x), Level-Set Contours, Safety Barrier B(x) = 0,
// and Sheaf-Coupled Autonomous Agent Trajectories converging to V(x*) = 0.

#let teal-deep = rgb("#083344")
#let teal-dark = rgb("#164e63")
#let teal = rgb("#0e7490")
#let cyan = rgb("#06b6d4")
#let cyan-light = rgb("#67e8f9")
#let cyan-ice = rgb("#cffafe")
#let amber = rgb("#d97706")
#let gold = rgb("#f59e0b")
#let gold-light = rgb("#fde68a")
#let crimson = rgb("#dc2626")

#let cover-illustration() = box(width: 240pt, height: 220pt)[
  // Outer Barrier Halo B(x) = 0 (dashed crimson/amber boundary ellipse at top of funnel)
  #place(
    dx: 10pt,
    dy: 10pt,
    ellipse(
      width: 220pt,
      height: 68pt,
      fill: cyan-ice.transparentize(65%),
      stroke: (paint: crimson.transparentize(20%), thickness: 1.5pt, dash: "dashed"),
    ),
  )

  // Funnel body polygon (shaded gradient-like layers from wide top to equilibrium apex)
  #place(
    top + left,
    polygon(
      fill: teal.transparentize(78%),
      stroke: (paint: teal-dark.transparentize(35%), thickness: 1pt),
      (12pt, 44pt),
      (68pt, 148pt),
      (120pt, 196pt),
      (172pt, 148pt),
      (228pt, 44pt),
    ),
  )

  // Descending Lyapunov level-set ellipses V(x) = c_k
  #for (dy, w, h, col, sw) in (
    (22pt, 196pt, 58pt, teal, 1.1pt),
    (52pt, 158pt, 46pt, teal, 1.1pt),
    (82pt, 120pt, 34pt, cyan, 1.2pt),
    (112pt, 84pt, 24pt, cyan, 1.3pt),
    (142pt, 50pt, 14pt, gold, 1.5pt),
  ) {
    place(
      dx: 120pt - w / 2,
      dy: dy,
      ellipse(
        width: w,
        height: h,
        fill: col.transparentize(88%),
        stroke: (paint: col, thickness: sw),
      ),
    )
  }

  // Sheaf cohomology stalk coupling chords across the funnel
  #for (p1, p2) in (
    ((42pt, 54pt), (198pt, 54pt)),
    ((58pt, 82pt), (182pt, 82pt)),
    ((76pt, 110pt), (164pt, 110pt)),
    ((94pt, 138pt), (146pt, 138pt)),
  ) {
    place(
      top + left,
      line(
        start: p1,
        end: p2,
        stroke: (paint: teal-dark.transparentize(45%), thickness: 0.8pt, dash: "dotted"),
      ),
    )
  }

  // Stabilized agent trajectories spiraling down into the equilibrium origin V(x*) = 0
  #let trajectories = (
    ((28pt, 38pt), (48pt, 72pt), (72pt, 108pt), (96pt, 146pt), (120pt, 192pt)),
    ((212pt, 38pt), (192pt, 72pt), (168pt, 108pt), (144pt, 146pt), (120pt, 192pt)),
    ((64pt, 24pt), (78pt, 64pt), (92pt, 104pt), (106pt, 148pt), (120pt, 192pt)),
    ((176pt, 24pt), (162pt, 64pt), (148pt, 104pt), (134pt, 148pt), (120pt, 192pt)),
    ((120pt, 16pt), (120pt, 62pt), (120pt, 108pt), (120pt, 152pt), (120pt, 192pt)),
  )
  #for tr in trajectories {
    place(
      top + left,
      curve(
        stroke: (paint: teal-deep, thickness: 1.7pt),
        curve.move(tr.at(0)),
        ..tr.slice(1).map(pt => curve.line(pt)),
      ),
    )
    // Agent node markers along trajectory
    for pt in tr.slice(0, 4) {
      place(
        dx: pt.at(0) - 3pt,
        dy: pt.at(1) - 3pt,
        circle(radius: 3pt, fill: cyan-light, stroke: 0.9pt + teal-deep),
      )
    }
  }

  // Equilibrium attractor core V(x*) = 0 at apex (120pt, 192pt)
  #place(
    dx: 120pt - 12pt,
    dy: 192pt - 12pt,
    circle(radius: 12pt, fill: gold-light.transparentize(35%), stroke: 1.2pt + amber),
  )
  #place(
    dx: 120pt - 6.5pt,
    dy: 192pt - 6.5pt,
    circle(radius: 6.5pt, fill: gold, stroke: 1.2pt + teal-deep),
  )
]

#let cover-art = cover-illustration
