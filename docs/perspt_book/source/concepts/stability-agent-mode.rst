.. _stability-agent-mode:

Stability Papers in Agent Mode
==============================

Perspt's agent mode is the first executable surface where the three
*Stability is All You Need* papers are being turned into product machinery. The
central rule is plain: a model may propose an action, but Perspt must measure
the proposed state before it accepts it.

The papers do not ask the reader to trust generation. They describe a discipline
for long-running agents: observe each candidate, compute residuals, correct the
dominant residuals through independent barriers, and commit only an accepted
trajectory to durable history. PSP-8 carries that discipline from one coding
agent toward an SDK-first platform with reusable stability contracts.

.. admonition:: Scope of claim
   :class: note

   This chapter describes how Perspt implements and plans around the SRBN
   contracts. The mathematical claims belong to the papers and their stated
   assumptions. Perspt's implementation is an active engineering system; it is
   not presented here as a completed empirical proof of those claims.

The Three Contracts
-------------------

Paper I: stable state
~~~~~~~~~~~~~~~~~~~~~

Paper I, *Stability is All You Need: Lyapunov-Guided Hierarchies for
Long-Horizon LLM Reliability*, gives the basic SRBN certificate. It treats an
agent run as a sequence of candidate states and asks whether accepted states
move toward a verified manifold.

In Perspt, the corresponding object is a DAG node. The Actuator may produce an
artifact bundle, but the bundle is still only a proposal. It becomes accepted
state only after verification measures its residual energy and the node reaches
the configured acceptance rule.

Paper II: observed harness
~~~~~~~~~~~~~~~~~~~~~~~~~~

Paper II, *Stability is All You Need II: SRBN-Control Beyond Harness
Engineering*, turns the certificate into a harness contract. Every candidate is
observed. A candidate may enter the accepted trajectory only when it hard-passes
the checks or descends under the measured energy. Exhaustion must end with a
residual certificate, not with a success claim.

In Perspt, PSP-7 and PSP-8 express this through parse states, retry classes,
correction attempt records, energy snapshots, and terminal outcomes visible
through ``perspt status`` and the dashboard.

Paper III: platform control
~~~~~~~~~~~~~~~~~~~~~~~~~~~

Paper III, *Stability is All You Need III: The SRBN Platform Contract*, moves
from a single harness to a platform. Stochastic actors emit proposals;
deterministic capabilities mediate effects; authority can only be scoped down;
risk budgets and recovery decisions are recorded; replay must be deterministic.

In Perspt, PSP-8 maps this idea to an SDK/domain split. The SRBN control plane
should be reusable. Coding, research, website building, and other domains should
provide their own sensors, residuals, verifier suites, artifacts, and admissible
effects.

The Energy That Agent Mode Measures
-----------------------------------

Perspt uses a concrete Lyapunov-style score for coding work. The verifier
combines syntax, structure, tests, bootstrap, and cross-node consistency:

.. math::

   \begin{aligned}
   V(x) ={}& \alpha V_{syn}(x) + \beta V_{str}(x)
             + \gamma V_{log}(x) \\
           & + V_{boot}(x) + V_{sheaf}(x).
   \end{aligned}

The default weights are :math:`\alpha = 1.0`, :math:`\beta = 0.5`, and
:math:`\gamma = 2.0`. The default acceptance threshold is
:math:`\varepsilon = 0.10`.

The important point is not the arithmetic alone. The score gives the controller
a memory of direction. A retry is useful only when it is tied to evidence about
what remains broken.

The acceptance rule can be read as follows:

.. math::

   \operatorname{accept}(x_{t+1}) =
   \begin{cases}
   \mathrm{true}, & V(x_{t+1}) \leq \varepsilon, \\
   \mathrm{true}, & V(x_{t+1}) < V(x_t)\ \mathrm{and\ descent\ is\ allowed}, \\
   \mathrm{false}, & \mathrm{otherwise}.
   \end{cases}

How Agent Mode Uses The Contracts
---------------------------------

Agent mode incorporates the papers through five load-bearing mechanisms.

Accepted trajectory
   Generated text and accepted state are different things. The Actuator may
   propose a bundle, but the ledger records only stable nodes, escalations, and
   explicit outcomes.

Residual-directed verification
   LSP diagnostics, build commands, tests, contract checks, bootstrap commands,
   and sheaf validators become residual evidence. The controller uses that
   evidence to choose correction, graph revision, escalation, or commit.

Independent correction barriers
   Corrections are grounded in verifier output rather than blind retry.
   Diagnostic messages, failing tests, parse failures, rejected bundles, and
   cross-node consistency failures are rendered into typed correction prompts.

Ledgered history
   Accepted nodes, correction attempts, energy snapshots, approvals, and session
   outcomes are written to the DuckDB-backed Merkle ledger. A committed run must
   be inspectable, resumable, and auditable.

Plugin-provided sensors
   Coding-domain language plugins provide project detection, LSP selection,
   syntax and build checks, test commands, bootstrap commands, and verifier
   profiles. In PSP-8, these plugins become adapters over the SRBN SDK instead
   of containers for the whole control plane.

What PSP-8 Adds
---------------

PSP-8 reframes Perspt from a hardcoded coding agent into an SRBN agent platform.
The coding agent remains the first domain package, but the shared stability
machinery moves behind SDK contracts.

.. list-table:: PSP-8 platform contracts
   :header-rows: 1
   :widths: 28 72
   :class: longtable

   * - Contract
     - Purpose
   * - SRBN kernel adapter
     - Standardize stabilization loops, barrier results, attempt traces,
       descent gates, and terminal statuses.
   * - Mutable work graph
     - Let repair actions split nodes, insert interface nodes, reset subgraphs,
       and schedule newly generated work instead of walking one fixed snapshot.
   * - Residual taxonomy
     - Explain verifier failures in typed form and attach correction directions
       to compiler, LSP, AST, import, test, policy, and logical residuals.
   * - Capability kernel
     - Treat model output as proposals. File writes, command execution, network
       access, and durable commits must pass scoped capability and policy checks.
   * - Replayable ledger
     - Record proposals, rejections, approvals, effects, observations, graph
       revisions, and rollbacks so sessions can be replayed and audited.
   * - Dashboard projection
     - Expose graph, residual, capability, worker, verifier, budget, and replay
       state without coupling the UI to coding-specific internals.

Improvement Roadmap
-------------------

The next Perspt improvements should make the implementation closer to the
papers' measurable contracts.

1. Replace stage-only verification summaries with first-class residual vectors.
   Each vector should name the failed invariant, its evidence, its severity, and
   the recommended correction direction.
2. Make repair actions durable scheduler commands. Graph rewrites, retries,
   splits, and inserted nodes should be executed by the active work graph.
3. Add cheap read-only repository exploration before planning unfamiliar
   codebases. The Architect should receive witnesses, not a blind map of every
   file.
4. Parallelize independent scans, symbol extraction, verifier runs, and unrelated
   node execution while preserving explicit dependency edges and commit ordering
   for conflicting durable effects.
5. Route correction prompt families through the typed prompt compiler, with
   provenance from residual evidence to rendered prompt.
6. Strengthen replay until a session can be reconstructed from ledger events:
   proposals, admissibility decisions, verifier observations, accepted states,
   escalations, and recovery actions.

Plugin Roadmap
--------------

Future plugins should be domain adapters over the SDK, not standalone
orchestrators. A complete coding adapter should provide:

* project detection and initialization commands;
* parser or AST extraction;
* symbol and import graph inventory;
* LSP, formatter, build, and test sensors;
* dependency and manifest mutation policies;
* residual classes and correction-direction mappers;
* benchmark fixtures that test descent, recovery, and replay behavior.

Near-term coding adapters should deepen Rust, Python, and TypeScript support,
then add Go and other languages where deterministic tooling is strong. Beyond
coding, the same SDK can support research plugins with citation and claim
residuals, website-builder plugins with accessibility and responsive-layout
residuals, and task-specific plugins whose verifiers define their own safe
manifolds.

The principle is constant: a plugin should not merely tell the model what to do.
It should define what can be measured, what counts as a residual, what
corrections are admissible, and what evidence is required before Perspt commits
state.

.. seealso::

   :doc:`srbn-architecture`
      Technical details of the current SRBN coding agent.

   :doc:`../developer-guide/extending`
      Developer guidance for language plugins and future domain adapters.