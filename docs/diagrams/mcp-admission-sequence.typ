#import "style.typ": *

#show: diagram-page.with(width: 740pt, margin: 16pt)

#section-banner(
  "Model Context Protocol (MCP 2026-07-28) Governance",
  "Capability-Bounded MCP Admission, Bracketed Effects, and Deterministic Replay",
  subtitle: [External MCP servers never self-grant authority. Every remote tool is admitted against the intersection of operator `ExternalToolPolicy` and session `Capability`, wrapped as an untrusted observation, and bracketed in `psp9_external_effects`.],
)

#v(8pt)

#grid(
  columns: (1fr, 1fr, 1fr, 1fr),
  gutter: 8pt,
  card(
    "1. Protocol & Discovery",
    [
      `ExternalToolRuntime` connects via `Stdio` or `StreamableHttp` (redirects disabled) and requires protocol `2026-07-28` via `server/discover` before `tools/list`.
    ],
    kicker: "Transport Layer",
    accent-color: teal,
  ),
  card(
    "2. Triple Admission Gate",
    [
      Each remote tool must have an operator `ExternalToolPolicy` (`effect`, `risk`, `footprint`). Undeclared tools default to `RunShell + Opaque` and are rejected if withheld.
    ],
    kicker: "Zero Self-Grant",
    accent-color: crimson,
  ),
  card(
    "3. Bracketed Execution",
    [
      Before invoking `tools/call`, the runtime records `intent_hash` in `psp9_external_effects` at the active authority epoch and prefixes output with `[untrusted MCP observation]`.
    ],
    kicker: "Durable Idempotency",
    accent-color: amber,
  ),
  card(
    "4. Offline Ledger Replay",
    [
      During `perspt replay` and `perspt resume`, recorded `ExternalToolResult` entries answer deterministically by canonical `replay_key_for(tool, args)` without respawning servers.
    ],
    kicker: "Deterministic Replay",
    accent-color: emerald,
  ),
)

#v(8pt)

#block(
  width: 100%,
  fill: panel,
  stroke: 0.8pt + rule,
  radius: 4pt,
  inset: 10pt,
  [
    #text(size: 8.2pt, weight: "black", tracking: 0.08em, fill: ink)[GOVERNED MCP ADMISSION & EXECUTION PIPELINE]
    #v(6pt)
    #grid(
      columns: (1.1fr, 18pt, 1.25fr, 18pt, 1.25fr, 18pt, 1.2fr),
      align: (left + horizon, center + horizon, left + horizon, center + horizon, left + horizon, center + horizon, left + horizon),
      block(
        width: 100%,
        fill: paper,
        stroke: 0.75pt + teal,
        radius: 3pt,
        inset: 7pt,
        [
          #badge("MCP Server", fg: teal-dark, bg: teal-soft, border: teal)
          #v(3pt)
          #text(size: 7.6pt, weight: "bold")[Declares Remote Tools]
          #v(2pt)
          #text(size: 7pt, fill: ink-Mid)[
            • `echo` (Read text)\
            • `fetch` (Network URL)\
            • `shell` (Undeclared)
          ]
        ],
      ),
      text(size: 11pt, weight: "black", fill: teal)[$->$],
      block(
        width: 100%,
        fill: paper,
        stroke: 0.75pt + amber,
        radius: 3pt,
        inset: 7pt,
        [
          #badge("Policy × Capability", fg: amber-dark, bg: amber-soft, border: amber)
          #v(3pt)
          #text(size: 7.6pt, weight: "bold")[Admission Filter]
          #v(2pt)
          #text(size: 7pt, fill: ink-Mid)[
            • `echo`: `Search` $in$ Cap (#text(fill: emerald, weight: "bold")[ADMIT])\
            • `fetch`: `NetworkFetch` withheld (#text(fill: crimson, weight: "bold")[REJECT])\
            • `shell`: `RunShell` default (#text(fill: crimson, weight: "bold")[REJECT])
          ]
        ],
      ),
      text(size: 11pt, weight: "black", fill: amber)[$->$],
      block(
        width: 100%,
        fill: paper,
        stroke: 0.75pt + violet,
        radius: 3pt,
        inset: 7pt,
        [
          #badge("Bracketed Call", fg: violet-dark, bg: violet-soft, border: violet)
          #v(3pt)
          #text(size: 7.6pt, weight: "bold")[`mcp.fixture.echo`]
          #v(2pt)
          #text(size: 7pt, fill: ink-Mid)[
            1. Record `pending` intent\
            2. Execute `tools/call`\
            3. Seal `completed` result
          ]
        ],
      ),
      text(size: 11pt, weight: "black", fill: emerald)[$->$],
      block(
        width: 100%,
        fill: paper,
        stroke: 0.75pt + emerald,
        radius: 3pt,
        inset: 7pt,
        [
          #badge("Merkle Replay", fg: emerald-dark, bg: emerald-soft, border: emerald)
          #v(3pt)
          #text(size: 7.6pt, weight: "bold")[Offline Verification]
          #v(2pt)
          #text(size: 7pt, fill: ink-Mid)[
            Replays exact recorded observation from `psp9_external_effects`; refuses unrecorded calls.
          ]
        ],
      ),
    )
  ],
)
