.. _srbn-architecture:

SRBN Architecture
=================

The **Stabilized Recursive Barrier Network (SRBN)** is the idea behind Perspt's
experimental autonomous coding agent. It comes from the *Stability is All You
Need* paper series.

The idea is plain to state. A language model is good at proposing changes, but a
proposal is only a guess. Left unchecked, a long run of guesses drifts: small
mistakes pile up until the work is broken. SRBN refuses to trust a guess on its
word. It measures every proposed change, keeps the change only when the
measurement shows progress, and writes the kept result to a permanent record.
Perspt's coding agent implements this work via a reusable software-development-kit
(SDK) platform (originally specified in the references section under PSP-8).

.. admonition:: Theory vs. Implementation
   :class: note

    This page describes both the SRBN paper series and how Perspt's
    runtime implements it. Where a claim comes from the papers' formal proofs,
    it is noted as a **paper result**. Where the developer team makes engineering choices that
    approximate or extend the theory, those are noted as **implementation details**.
    The theoretical framework is mature; empirical benchmarks on Perspt's implementation
    have not yet been published.

Overview
--------

The SRBN paper models coding tasks as a directed acyclic graph (DAG) of nodes with
a sheaf structure that enforces consistency across shared boundaries. The system
establishes the following concrete mechanics: each node owns a set
of output files (ownership closure), runs a governed tool loop of typed tool
calls against an isolated candidate, and must pass the measured acceptance
gate on deterministic verifier evidence. Only then is the node's winner
promoted and committed to the Merkle ledger.

The runtime environment structures execution in two primary ways:

- **Quadratic energy.** Acceptance is gated on the quadratic residual energy
  :math:`V(x) = \sum_{e} w_e \lVert r_e \rVert^2` (see `Lyapunov Energy`_ below).
- **Mutable work graph.** Rather than walking a precomputed topological order, a
  closed-loop scheduler re-evaluates a dependency-aware ready queue each round and
  may requeue, split, insert, or replan nodes as verifier evidence arrives.
  Each individual graph *revision* stays acyclic.
  Bounded parallelism shipped in v0.6.6: the multi-node dispatcher behind
  ``--max-parallel-nodes`` runs ready nodes concurrently, and two nodes with
  conflicting file footprints never run at the same time.

The core concepts of the control system — ownership closure, typed
tool calls, the verifier profiles, node classes, and the Merkle ledger —
are described below.

.. graphviz::
   :align: center
   :caption: SRBN Architecture

   digraph srbn {
       rankdir=TB;
       compound=true;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10];

       subgraph cluster_models {
           label="Model Roles";
           style=dashed;
           arch [label="Architect\n(Graph Planning)", fillcolor="#E8F5E9"];
           act [label="Actuator\n(Governed Tool Calls)", fillcolor="#E3F2FD"];
           exp [label="Explorer\n(Read-Only Exploration)", fillcolor="#FFF3E0"];
           adj [label="Adjudicator\n(No-Tool Diff Review)", fillcolor="#F3E5F5"];
       }

       kernel [label="Admissibility Kernel\n(Deterministic)", fillcolor="#D1C4E9"];
       cand [label="Candidate Overlay", fillcolor="#E0E0E0"];

       subgraph cluster_barriers {
           label="Deterministic Verifier Sensors";
           style=dashed;
           lsp [label="V_syn\n(LSP)", fillcolor="#FFECB3"];
           tests [label="V_log\n(Tests)", fillcolor="#FFECB3"];
           boot [label="V_boot\n(Build)", fillcolor="#FFECB3"];
           struct [label="V_str\n(Contracts)", fillcolor="#FFECB3"];
           sheaf [label="V_sheaf\n(Cross-Node)", fillcolor="#FFECB3"];
       }

       gate [label="Accept Gate\n(hard pass or descent)", fillcolor="#B3E5FC"];

       subgraph cluster_output {
           label="Output";
           style=dashed;
           ledger [label="Merkle Ledger\n(DuckDB)", fillcolor="#C8E6C9"];
       }

       arch -> act [label="work graph"];
       exp -> act [label="evidence", style=dashed];
       act -> kernel [label="typed proposals"];
       kernel -> cand [label="admitted effects"];
       cand -> lsp;
       cand -> tests;
       cand -> boot;
       cand -> struct;
       cand -> sheaf;
       lsp -> gate;
       tests -> gate;
       boot -> gate;
       struct -> gate;
       sheaf -> gate;
       adj -> gate [label="conjunctive review", style=dashed];
       gate -> act [label="correction", style=dashed];
       gate -> ledger [label="promote"];
   }


The Control Loop
----------------

Bootstrap and the architect turn run once at the start; the remaining phases
run **per node inside the closed loop**. The dispatcher re-evaluates the
mutable work graph, fills free slots with ready nodes, and runs the governed
tool loop for each. A node that fails its gate can open a bounded search
forest and be re-attempted, so these phases are *not* a single topological
pass:

.. list-table::
   :header-rows: 1
   :widths: 5 20 75

   * - #
     - Phase
     - Description
   * - 1
     - **Session Bootstrap**
     - Create the durable session, detect the domain package and language
       plugins, mint epoch-bound capabilities from the grant policy, and
       compile the prompt programs for the resolved routes and dialects.
   * - 2
     - **Architect Turn**
     - A governed, forced-tool-choice model turn restricted to the privileged
       ``update_graph`` tool proposes the work-graph revision. Acyclicity and
       completeness are validated deterministically before the revision is
       accepted; a single-node run keeps one node without an architect call.
   * - 3
     - **Work-Graph Dispatch**
     - Behind ``--max-parallel-nodes``, the dispatcher fills free slots with
       ready nodes. Conflicting file footprints never run concurrently, and
       each node attempt is bound to a generation seeded from the current
       staging root, never from unstaged sibling state.
   * - 4
     - **Governed Tool Loop**
     - Per node, model turns issue typed tool calls. Each call becomes a
       proposal; the deterministic admissibility kernel decides whether it
       may affect the candidate; admitted effects mutate an isolated
       candidate overlay; sandboxed deterministic verifiers re-measure the
       candidate.
   * - 5
     - **Accept Gate**
     - A candidate is accepted on a hard verifier pass, or on a measured
       energy descent of at least :math:`\rho_{\text{gate}}` below the best
       accepted state. Non-descending candidates consume the shared
       rejection budget; the gate is evaluated on the re-measured candidate,
       never on the model's account of it.
   * - 6
     - **Bounded Search** (optional)
     - On gate failure with budget remaining, a bounded search forest runs
       up to three isolated branches with distinct strategies against the
       accepted root. Every branch action reserves its cost before it runs,
       exact no-goods suppress repeats, and exactly one selected candidate
       is committed through the same gate.
   * - 7
     - **Staging & Integration**
     - Each node's winner lands in a content-addressed staging root instead
       of the user workspace. When dispatch settles, the merged state passes
       one global integration gate; failure restores the prior staging root.
   * - 8
     - **Promotion**
     - The integrated winner is written to the user workspace through
       journaled, descriptor-relative promotion and committed to the
       hash-chained ledger, from which ``perspt replay`` and
       ``perspt resume`` reconstruct the session.


Lyapunov Energy
---------------

The stability of generated code is measured using a Lyapunov energy function, adapted
from the paper's sheaf-theoretic formulation into concrete verification barriers. The
system evaluates a canonical **quadratic residual energy**: each sensor :math:`e`
emits a residual with a non-negative magnitude :math:`r_e(x)`, and the energy is a weighted
sum of their squares.

Let :math:`\mathcal{E}` be the set of active sensors monitoring the system. For a proposed
state :math:`x`, the total Lyapunov energy :math:`V(x)` is defined by the quadratic form:

.. math::

   V(x) = \sum_{e \in \mathcal{E}} w_e \, \lVert r_e(x) \rVert^2

where :math:`w_e > 0` represents the positive weight assigned to the residual class of
sensor :math:`e`. 

The five component readouts :math:`V_{syn}`, :math:`V_{str}`, :math:`V_{log}`,
:math:`V_{boot}`, and :math:`V_{sheaf}` are **derived rollups** of this same energy,
grouped by component type, such that the total is the sum of the rollups:

.. math::

   V(x) = V_{\text{syn}} + V_{\text{str}} + V_{\text{log}} + V_{\text{boot}} + V_{\text{sheaf}}

In this formulation, the rollups themselves carry the squared, weighted
residual contributions:

.. math::

   V_{\text{comp}} = \sum_{e \in \text{comp}} w_e \, \lVert r_e(x) \rVert^2

There is no secondary :math:`\alpha/\beta/\gamma` aggregation pass. The
per-class weights :math:`w_e` are fixed by the domain package's energy model,
leaving the core mathematical engine as a pure sum of pre-weighted squares.

Components
~~~~~~~~~~

We enumerate the five component categories and the specific residual classes mapped
to them from the `perspt-coding` domain package:

.. list-table::
   :header-rows: 1
   :widths: 15 25 60

   * - Component
     - Mapped Residual Classes (Weights)
     - Operational Semantics
   * - **V_syn**
     - * ``Syntax`` (weight: 4.0)
       * ``Type`` (weight: 3.0)
       * ``Build`` (weight: 3.0)
     - Captures syntax errors, build-time compilation failures, and LSP diagnostic warnings.
       Compiler warnings or type-check diagnostics produce residuals of class ``Type``,
       where the magnitude is the raw diagnostic count. A build failure raises a blocking
       residual of class ``Build``.
   * - **V_str**
     - * ``ImportGraph`` (weight: 2.0)
       * ``SymbolMismatch`` (weight: 2.0)
       * ``InterfaceMismatch`` (weight: 2.5)
       * ``OwnershipViolation`` (weight: 2.0)
       * ``Policy`` (weight: 1.0)
       * ``Dependency`` (weight: 1.5)
       * ``Manifest`` (weight: 1.5)
       * ``Format`` (weight: 0.25)
     - Measures structural contract adherence. If a required symbol defined by a node's public
       interface signature is absent from the generated output files, the ``GoalPresence`` sensor
       registers a blocking ``SymbolMismatch`` residual, preventing convergence. Policy and format
       irregularities are likewise squared into this component.
   * - **V_log**
     - * ``TestFailure`` (weight: 2.0)
       * ``Runtime`` (weight: 2.0)
       * ``Regression`` (weight: 3.0)
     - Evaluates behavioral outcomes. Unit test failures yield a ``TestFailure`` residual with a
       magnitude matching the count of failing test cases. Post-build runtime smoke probes detect
       process crashes, tracebacks, or numeric anomalies, adding a ``Runtime`` residual.
   * - **V_boot**
     - * ``SensorUnavailable`` (weight: 1.0)
       * ``ToolFailure`` (weight: 1.0)
     - Registers system infrastructure state. If a required sensor (e.g., an LSP server or
       test runner) is degraded or unavailable, it raises a ``SensorUnavailable`` residual,
       forcing the energy high so the system knows its stability status is indeterminate.
   * - **V_sheaf**
     - * ``SheafInconsistency`` (weight: 2.0)
     - Evaluates global structural consistency across nodes. Cross-node import/dependency
       mismatches or signature differences raise a ``SheafInconsistency`` residual.

Convergence Criterion
~~~~~~~~~~~~~~~~~~~~~

The system is considered stable if and only if the candidate achieves a
**hard pass**: every required verifier and every hard policy constraint
passes. A domain may additionally declare an analytic energy floor; a
candidate at or below the floor is checkpointed as a classified terminal
state but is never reported as verified success. If the state is not stable,
the accept-gate evaluates descent. The single runtime acceptance knob is
``--rho-gate``. A state :math:`x` can be provisionally accepted during
convergence if:

.. math::

   V(x) \leq V(x_{\text{best}}) - \rho_{\text{gate}}

where :math:`x_{\text{best}}` is the best previously accepted state, and :math:`\rho_{\text{gate}}`
is the minimum descent gate (default :math:`0.50`), ensuring non-trivial progress.

Spectral and Independence Diagnostics
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The system supports an energy-slope constant :math:`\mu = 2\,\lambda_{\min}^{+}(A)`
— twice the algebraic connectivity (Fiedler value) of the verification graph
built from the quadratic energy :math:`V(x)=x^{\top}Ax`. It measures how
strongly the verifier ensemble drives the code toward consensus, and its
sensitivity to a candidate verifier edge distinguishes an *independent* verifier
(which raises the spectral gap) from a *redundant* one. A companion measure,
:math:`\rho_{\text{eff}}`, captures effective verifier independence from observed
miss correlations.

.. note::

   ``mu`` and :math:`\rho_{\text{eff}}` are **diagnostics, not acceptance-gate
   inputs** — by design they are computed off the critical path and never block a
   node. The independence statistics are **live**: ``independence::compute``
   folds the ``psp9_verdicts`` ledger into per-validator miss rates and
   matched-stratum pair statistics, and :math:`\rho_{\text{eff}}` is reported
   only when every pair meets the matched-sample floor (a Hoeffding upper
   confidence bound backs the certified value). ``perspt status`` and the
   dashboard's Governance page surface these figures. The spectral
   eigensolver (``spectral::VerificationGraph``) remains an offline
   ``perspt-sdk`` diagnostic.


Node Classes
------------

The system utilizes three node classes that govern execution order and verification:

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Class
     - Description
   * - **Interface**
     - Define exported signatures, schemas, and seals. Must be committed before
       dependent Implementation nodes can proceed. Produces an interface-seal digest.
   * - **Implementation**
     - Operate on owned files using sealed interfaces from parent nodes. The bulk of
       code generation happens here.
   * - **Integration**
     - Reconcile cross-owner boundaries after all dependent nodes converge. Used for
       multi-language projects or cross-module wiring.


Ownership Closure
-----------------

The **ownership closure** rule is a fundamental invariant of PSP-5:

   *Each output file appears in exactly one node's* ``output_files`` *list.*

This prevents conflicting writes. In the current runtime the guarantee is
enforced at dispatch through footprints: two nodes whose declared file
footprints conflict never run concurrently, and each node attempt is seeded
from the staging root rather than from unstaged sibling state.


Model Roles
-----------

The runtime uses one required and two optional model roles. Verification is
**not** a model role: the verifiers are deterministic sandboxed sensors
(compilers, test runners, linters), never a model call.

.. list-table::
   :header-rows: 1
   :widths: 20 45 35

   * - Role
     - Purpose
     - CLI Flag
   * - **Actuator**
     - Proposes governed coding tool calls; the only role that can mutate
       the candidate. Also serves the architect's ``update_graph`` turn.
     - ``--model`` / ``--actuator-model``
   * - **Explorer**
     - Optional cheaper read-only repository exploration.
     - ``--explorer-model``
   * - **Adjudicator**
     - Optional no-tool conjunctive diff adjudication before promotion.
     - ``--adjudicator-model``

Configure per-role routes via CLI:

.. code-block:: bash

   perspt agent \
     --actuator-model <provider::model> \
     --explorer-model <provider::model> \
     --adjudicator-model <provider::model> \
     --fallback-model <provider::model> \
     "Build a REST API"

``--fallback-model`` is repeatable and defines the ordered sticky actuator
failover route. Persistent per-role routes live in the ``[models]`` table of
``config.toml`` as fully qualified ``provider::model`` values, so identity,
calibration, and replay never depend on an ambient default provider. No
model name is hard-coded in the runtime.


PSP-10: Model-Conditioned Prompt Programs
-----------------------------------------

PSP-10 replaces free-form prompt templates with compiled prompt programs:
typed sections, deterministic composition per model route, and ledgered
digests for every model call.

Typed Prompt Sections
~~~~~~~~~~~~~~~~~~~~~

Prompt text lives in versioned section files under
``crates/perspt-core/prompts/<stage>/NN_name.md``, one directory per stage
(``session_bootstrap``, ``graph_plan``, ``repository_explore``,
``adjudicate``, ``evidence_summarize``), beside a committed
``manifest.toml``. The ``perspt-prompt-macros`` crate compiles every section
at build time from the owning crate's ``build.rs``: it parses the front
matter into a typed ``SectionSchema`` (id, version, role, priority, size
bound, declared variables), runs the codegen validation list, and emits typed section
structs into ``OUT_DIR``. A malformed section fails ``cargo build`` with an
error naming the offending file — never a session.

Stage Composition and Digests
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

At runtime a ``StageComposition`` composes one stage's rendered sections for
the resolved model route and dialect into a ``CompiledPromptProgram``: the
ordered messages with per-message rendered hashes, any sections dropped by
deterministic budget fitting, the tool-spec hash, and a ``program_digest``
computed over canonical bytes of the identity-bearing fields. One model call
binds a platform program (the universal envelope) and a domain program; the
pair forms a ``CompiledPromptInvocation`` with its own invocation digest.

Provenance
~~~~~~~~~~

Every model call is ledgered: a ``prompt_program_compiled`` event records
each program's sections and digest, and a ``prompt_program_invoked`` event
records the actor, turn, platform digest, domain digest, invocation digest,
and tool-spec hash. A session's prompts are therefore reconstructible and
diffable after the fact. Validated replacement bundles may substitute
section bodies live only behind ``--allow-experimental-prompts``; external
bodies obey exactly the validation rules the build does.

The ``perspt prompts`` CLI
~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   perspt prompts list                   # every compiled section: id, version, stage, role, hash
   perspt prompts render <stage>         # compose one stage with fixture variables
   perspt prompts lint --bundle <dir>    # validate an external bundle directory
   perspt prompts manifest <dir>         # regenerate a library's manifest.toml
   perspt prompts explain-session --db-path <db> <session-id>
                                         # the programs a session actually compiled, with digests

``perspt context explain-turn`` is the companion command for a session's
recorded resident-context events (compactions and refusals).


Rejection Budget
----------------

There is no per-error-type retry table. The loop runs against a single
shared, non-replenishing **rejection budget** :math:`B` (default 4,
``--rejection-budget``): non-descending candidates and recovery attempts
consume it, while accepted descents do not. Together with the descent gate
this yields the finite-decision bound
:math:`\lfloor V_0 / \rho_{\text{gate}} \rfloor + B + 1` on gate decisions
per node. When the budget is exhausted, the gate issues a **residual
certificate** enumerating the remaining errors and the node terminates as a
classified failure rather than looping. In headless mode (``--yes``) the
session exits with a non-zero code when the outcome is not verified success.


Typed Tool Proposals
--------------------

The Actuator does not emit artifact bundles. Every mutation is an individual
typed tool call: the call becomes an effect proposal, the deterministic
admissibility kernel checks it against the node's capability (path patterns,
command patterns, effect kinds), and only an admitted effect executes —
against the isolated candidate overlay, never the user workspace. Proposed
commands additionally pass the deterministic ``sanitize_command`` and
workspace-bound guards plus the Starlark policy engine, and run inside the
process sandbox. The candidate journals a pre-image of every file it
touches, so any attempt can be discarded without residue.


Plugin-Driven Verification
--------------------------

Language plugins determine the verification toolchain:

.. list-table::
   :header-rows: 1
   :widths: 15 20 20 20 25

   * - Plugin
     - LSP Server
     - Test Runner
     - Init Command
     - Required Binaries
   * - **Rust**
     - ``rust-analyzer``
     - ``cargo test``
     - ``cargo init``
     - ``cargo``, ``rustc``
   * - **Python**
     - ``ty`` or ``pyright``
     - ``pytest``
     - ``uv init --lib``
     - ``uv``, ``python3``
   * - **JavaScript**
     - ``typescript-language-server``
     - ``npm test``
     - ``npm init -y``
     - ``node``, ``npm``
   * - **Go**
     - ``gopls``
     - ``go test``
     - ``go mod init``
     - ``go``

The plugin is selected automatically during the Detection phase based on existing
project files or the task description. Multi-language projects activate multiple
plugins simultaneously.


Degraded Verification
---------------------

When a verification tool is unavailable (e.g., ``ty`` not installed), the SRBN
engine falls back to degraded mode:

- **Sensor fallback**: If the primary LSP server is not found, try a secondary
  (e.g., ``pyright`` instead of ``ty``). Emit a ``SensorFallback`` event.
- **Degraded stages**: If no LSP server is available at all, V_syn is set to 0.0
  and the stage is marked degraded. Energy convergence proceeds without that
  component.
- **Stability blocked**: If too many stages degrade, the node cannot converge and
  is escalated.


Merkle Ledger
-------------

All changes are recorded in a DuckDB-backed Merkle ledger:

- **Integrity** - Each commit has a cryptographic hash chaining to its parent
- **Rollback** - Roll back a session's newest completed promotion via
  ``perspt ledger --rollback`` (the argument is a session id prefix)
- **Resume** - ``perspt resume`` continues an interrupted session from its
  newest durable checkpoint by folding the ledger (see `Crash Resume`_)
- **Audit** - Complete trail of AI-generated changes with energy breakdowns

.. code-block:: bash

   perspt ledger --recent     # View recent commits
   perspt ledger --rollback abc123
   perspt ledger --stats      # Session statistics


Isolation and Resume
--------------------

Nothing speculative ever touches the user workspace. Three layers keep
observed work separated from accepted work, and crash resume preserves the
separation.

.. admonition:: Key Invariant
   :class: important

   Model-proposed work reaches the user workspace only through the accept
   gate, the staging root, and the global integration gate — in that order.
   Everything else is discarded without residue.

Candidate Overlays
~~~~~~~~~~~~~~~~~~

Each node attempt runs in a ``CandidateWorkspace`` overlay. Admitted effects
mutate the overlay; every touched file's pre-image is journaled; verifiers
measure the overlay, not the workspace. A rejected attempt is dropped
wholesale.

Test evidence follows an explicit policy. The ordinary ``evolving`` policy
measures the resulting implementation and resulting tests together, which is
necessary when a task intentionally changes a contract. A
``backward-compatible`` run adds a regression view with recognized historical
test files restored. An ``external-oracle`` run adds a separately supplied
acceptance overlay that the actuator does not promote. These are additional
test-stage views of the same realized candidate, not alternate paths around
the syntax, build, or test requirements.

Search Forest Branches
~~~~~~~~~~~~~~~~~~~~~~

A bounded search forest opens at most three branch identities, one branch
attempt at a time. Every branch runs the ordinary governed loop in an
isolated eager-copy workspace against the same immutable accepted root, and
its internal states stay private to the forest. Exactly one selected
candidate is committed through a single gate submission, and the committed
decision must equal the preview or the forest fails closed. Exact no-goods
learned from failed branches suppress equivalent later attempts, and every
branch action reserves its cost against the declared limit vector before it
runs.

Staging and the Integration Gate
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

With ``--max-parallel-nodes`` above 1, node winners land in a
content-addressed staging root instead of the user workspace. Disjoint
footprints are enforced at dispatch; the merged state must pass one global
integration gate before promotion, and a gate failure restores the prior
staging root. The integration gate uses the same session-bound test policy as
the node gates: resulting tests for ordinary evolving development, an
additional historical regression view for explicitly backward-compatible
work, or a separately protected external acceptance overlay.

Crash Resume
~~~~~~~~~~~~

``perspt resume`` folds the durable ledger rather than deserializing live
state. A single-node session re-enters its loop from the newest durable
candidate checkpoint with exactly the remaining budgets, its accepted
candidate rebuilt from content-addressed artifacts and a fresh capability
minted at the current authority epoch (a bumped epoch refuses the resume).
A multi-node session resumes through graph dispatch: the staging root is
rebuilt from the ledger and every winner still reaches the user workspace
only through the global integration gate.
