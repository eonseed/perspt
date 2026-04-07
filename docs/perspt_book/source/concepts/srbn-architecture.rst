.. _srbn-architecture:

SRBN Architecture
=================

The **Stabilized Recursive Barrier Network (SRBN)** is the theoretical framework behind
Perspt's experimental autonomous coding agent. SRBN is based on the paper *"Stability
is All You Need: Lyapunov-Guided Hierarchies for Long-Horizon LLM Reliability"*
by **Vikrant R. and Ronak R.** (pre-publication), which reformulates LLM agency as a
sheaf-theoretic control problem and proves Input-to-State Stability (ISS) under
persistent noise. Perspt's implementation of this framework is defined by **PSP-5**
(Perspt Specification Proposal 5).

.. admonition:: Theory vs. Implementation
   :class: note

   This page describes both the SRBN paper's theoretical model and how Perspt's
   PSP-5 runtime implements it. Where a claim comes from the paper's formal proofs,
   it is noted as a **paper result**. Where PSP-5 makes engineering choices that
   approximate or extend the theory, those are noted as **implementation details**.
   The theoretical framework is mature; empirical benchmarks on Perspt's implementation
   have not yet been published.

Overview
--------

The SRBN paper models coding tasks as a directed acyclic graph (DAG) of nodes with
a sheaf structure that enforces consistency across shared boundaries. PSP-5 implements
this model concretely: each node owns a set of output files (ownership closure),
generates a multi-artifact bundle, and must pass multi-stage verification before its
energy falls below the convergence threshold. Only then is the node committed to the
Merkle ledger.

.. graphviz::
   :align: center
   :caption: SRBN Architecture

   digraph srbn {
       rankdir=TB;
       compound=true;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10];

       subgraph cluster_tiers {
           label="Model Tiers";
           style=dashed;
           arch [label="Architect\n(Deep Reasoning)", fillcolor="#E8F5E9"];
           act [label="Actuator\n(Code Generation)", fillcolor="#E3F2FD"];
           ver [label="Verifier\n(Stability Check)", fillcolor="#F3E5F5"];
           spec [label="Speculator\n(Fast Lookahead)", fillcolor="#FFF3E0"];
       }

       subgraph cluster_barriers {
           label="Verification Barriers";
           style=dashed;
           lsp [label="V_syn\n(LSP)", fillcolor="#FFECB3"];
           tests [label="V_log\n(Tests)", fillcolor="#FFECB3"];
           boot [label="V_boot\n(Build)", fillcolor="#FFECB3"];
           struct [label="V_str\n(Contracts)", fillcolor="#FFECB3"];
           sheaf [label="V_sheaf\n(Cross-Node)", fillcolor="#FFECB3"];
       }

       subgraph cluster_output {
           label="Output";
           style=dashed;
           ledger [label="Merkle Ledger\n(DuckDB)", fillcolor="#C8E6C9"];
       }

       arch -> act;
       act -> lsp;
       act -> tests;
       act -> boot;
       act -> struct;
       lsp -> ver;
       tests -> ver;
       boot -> ver;
       struct -> ver;
       ver -> act [label="retry", style=dashed];
       ver -> sheaf [label="stable"];
       sheaf -> ledger [label="commit"];
   }


The Control Loop (PSP-5 Implementation)
---------------------------------------

The SRBN control loop as implemented by PSP-5 executes seven phases for each task:

.. list-table::
   :header-rows: 1
   :widths: 5 20 75

   * - #
     - Phase
     - Description
   * - 1
     - **Detection**
     - Inspect the repository. Select language plugins (Rust, Python, JS, Go) based on
       existing files or the task description. Each plugin provides an LSP server, test
       runner, and init command.
   * - 2
     - **Planning**
     - Architect model decomposes the task into a ``TaskPlan`` DAG. Each node has an ID,
       goal, context files, output files, dependencies, task type, and node class
       (Interface, Implementation, or Integration). The **ownership closure** rule
       ensures each output file appears in exactly one node. Planning is gated by
       ``PlanningPolicy``: ``LocalEdit`` skips the Architect and creates a single-node
       graph; all other policies run the full Architect decomposition.
       A **FeatureCharter** is auto-created with policy-derived limits
       (``max_modules``, ``max_files``, ``max_revisions``) before planning begins.
   * - 3
     - **Generation**
     - Actuator model generates a multi-artifact bundle per node. The bundle is a JSON
       structure with ``write``, ``diff``, and ``command`` operations. All files are
       written transactionally.
   * - 4
     - **Verification**
     - Compute five energy components: V_syn (LSP diagnostics), V_str (contract
       violations), V_log (test failures), V_boot (init/build exit codes), and V_sheaf
       (cross-node consistency). Total energy is V(x).
   * - 5
     - **Convergence**
     - If V(x) > epsilon, generate a grounded correction prompt containing the specific
       error messages and retry. Bounded by ``RetryPolicy`` per error type.
   * - 6
     - **Sheaf Validation**
     - After all nodes converge individually, run cross-node consistency checks.
       Validates import paths, shared type signatures, and interface-seal digests.
   * - 7
     - **Commit & Outcome**
     - Record each node's terminal state in the Merkle ledger. Nodes that converge
       (V(x) ≤ ε) are committed as ``Completed``; nodes whose retries are exhausted
       are recorded as ``Escalated``. After all nodes are processed, the orchestrator
       derives a ``SessionOutcome`` from completed/escalated counts: ``Success`` (all
       completed), ``PartialSuccess`` (some escalated), or ``Failed`` (none completed).
       Emit ``Complete`` event with the derived outcome.


Lyapunov Energy
---------------

The stability of generated code is measured using a Lyapunov energy function, adapted
from the paper's sheaf-theoretic formulation into five concrete verification barriers:

.. admonition:: Energy Formula
   :class: important

   .. math::

      V(x) = \alpha \cdot V_{syn} + \beta \cdot V_{str} + \gamma \cdot V_{log} + V_{boot} + V_{sheaf}

   Default weights: alpha = 1.0, beta = 0.5, gamma = 2.0

Components
~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 15 25 60

   * - Component
     - Source
     - Description
   * - **V_syn**
     - LSP Diagnostics
     - Count of errors and warnings from the language server (``rust-analyzer``, ``ty``,
       ``pyright``, ``typescript-language-server``, ``gopls``).
   * - **V_str**
     - Contract Verification
     - Violations of ``BehavioralContract`` constraints: interface signatures, invariants,
       and forbidden patterns.
   * - **V_log**
     - Test Failures
     - Weighted sum of test failures. Critical tests carry weight 10, high-priority 3,
       low-priority 1. Computed via pytest or ``cargo test``.
   * - **V_boot**
     - Bootstrap Commands
     - Non-zero exit codes from init commands (``uv init --lib``, ``cargo init``),
       build commands, and dependency installs.
   * - **V_sheaf**
     - Cross-Node Consistency
     - Failures from sheaf validators: import-path resolution, shared-type agreement,
       and interface-seal digest mismatches.

Convergence Criterion
~~~~~~~~~~~~~~~~~~~~~

The system is considered stable when:

.. math::

   V(x) \leq \varepsilon

Default: epsilon = 0.10. Configurable via ``--stability-threshold``.


Node Classes
------------

PSP-5 introduces three node classes that govern execution order and verification:

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

This prevents conflicting writes. When the Architect generates a task plan, the
orchestrator validates ownership closure before execution begins. If two nodes
claim the same file, the plan is rejected and re-generated.


Model Tiers
-----------

SRBN uses four specialized model tiers. Each tier can be configured independently:

.. list-table::
   :header-rows: 1
   :widths: 20 30 50

   * - Tier
     - Purpose
     - Default Model
   * - **Architect**
     - Deep reasoning, task decomposition, DAG planning
     - ``gemini-pro-latest``
   * - **Actuator**
     - Code generation, artifact bundle emission
     - ``gemini-3.1-flash-lite-preview``
   * - **Verifier**
     - LSP diagnostics, contract checking, energy computation
     - ``gemini-pro-latest``
   * - **Speculator**
     - Fast lookahead, provisional branch prediction
     - ``gemini-3.1-flash-lite-preview``

Configure per-tier models via CLI:

.. code-block:: bash

   perspt agent \
     --architect-model gemini-pro-latest \
     --actuator-model gemini-3.1-flash-lite-preview \
     --verifier-model gemini-pro-latest \
     --speculator-model gemini-3.1-flash-lite-preview \
     "Build a REST API"

Each tier also supports a fallback model (``--architect-fallback-model``, etc.).


Planning Policy
---------------

The ``PlanningPolicy`` enum adapts the agent phase stack based on task scale:

.. list-table::
   :header-rows: 1
   :widths: 25 15 15 45

   * - Policy
     - Architect
     - Speculator
     - Description
   * - **LocalEdit**
     - No
     - No
     - Small, localized changes. Skips task decomposition; uses a single-node graph.
   * - **FeatureIncrement** (default)
     - Yes
     - No
     - Mid-size features. Architect decomposes, Actuator implements, Verifier checks.
   * - **LargeFeature**
     - Yes
     - Yes
     - Full SRBN loop including speculator lookahead for downstream risk hints.
   * - **GreenfieldBuild**
     - Yes
     - Yes
     - New project. Full stack with workspace bootstrap node first.
   * - **ArchitecturalRevision**
     - Yes
     - Yes
     - Cross-cutting redesign. Plan-first with speculator risk analysis.


PSP-7: Robust Correction Loop Contracts
-----------------------------------------

PSP-7 extends the SRBN runtime with three hardening layers: a typed parse pipeline,
a prompt compiler with provenance tracking, and structured correction telemetry.

Typed Parse Pipeline (Fail-Closed Parsing)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

LLM responses are processed through a five-layer typed pipeline that replaces the
legacy ``extract_all_code_blocks_from_response()`` fallback. Each layer returns a
``ParseResultState`` that classifies the outcome:

.. list-table::
   :header-rows: 1
   :widths: 15 85

   * - Layer
     - Description
   * - **A (Raw Capture)**
     - Hash, length, and first-line fingerprint of the raw response.
   * - **B (Path Normalization)**
     - Strip backticks, quotes, and markdown formatting from file paths.
   * - **C (Strict JSON)**
     - Attempt to parse the response as a structured JSON artifact bundle.
   * - **D (Tolerant Recovery)**
     - Recognize ``### File:``, ``File:``, and ``Diff:`` headings to extract
       structured content. Never invents filenames — unnamed blocks produce ``None``.
   * - **E (Semantic Validation)**
     - Plugin-driven ownership closure, ``legal_support_files()`` checks,
       ``dependency_command_policy()`` enforcement.

``ParseResultState`` has six variants: ``StructuredOk``, ``TolerantRecoveryOk``,
``NoStructuredPayload``, ``SchemaInvalid``, ``SemanticallyRejected``, ``EmptyResponse``.
Each variant carries a ``RetryClassification`` that guides the correction loop:
``Retarget``, ``SupportFiles``, ``Replan``, or ``FatalBudget``.

Active V_boot
~~~~~~~~~~~~~

PSP-7 separates bootstrap failures from code errors. After auto-repair re-verification,
if the verifier profile is fully degraded or missing-crate/module failures persist,
``V_boot`` is set independently rather than being folded into ``V_syn``. This gives
the correction loop a dedicated signal for infrastructure problems.

Sheaf Pre-Check
~~~~~~~~~~~~~~~

After a node converges (V(x) ≤ ε) but before the full sheaf validation pass, a fast
structural pre-check verifies that output artifacts declare consistent imports and
exports with the ownership manifest. If the pre-check fails, the node re-enters
``step_converge()`` with sheaf-specific evidence. A retry guard (max 1 sheaf pre-check
retry) prevents infinite loops.

Prompt Compiler
~~~~~~~~~~~~~~~

PSP-7 replaces the template-constant approach with a typed prompt compiler:

.. code-block:: text

   compile(intent: PromptIntent, evidence: &PromptEvidence) -> CompiledPrompt

The compiler accepts 13 ``PromptIntent`` variants (architect, actuator, verifier,
correction, speculator, solo, bundle retarget, project naming) and emits a
``CompiledPrompt`` with the assembled prompt text plus ``PromptProvenance`` metadata
(template ID, evidence hashes, compiler version). Plugin ``correction_prompt_fragment()``
and ``legal_support_files()`` are injected into correction prompts automatically.

Correction Telemetry
~~~~~~~~~~~~~~~~~~~~

Every correction attempt is recorded as a ``CorrectionAttemptRow`` in the DuckDB store:

- ``parse_state`` — which ``ParseResultState`` was returned
- ``retry_classification`` — how the failure was classified
- ``response_fingerprint`` — hash of the raw LLM response
- ``response_length`` — byte length for detecting degenerate responses
- ``energy_json`` — energy components snapshot after verification
- ``accepted`` / ``rejection_reason`` — whether the attempt was committed

Additionally, ``srbn_step_records`` track per-node execution steps (speculate, verify,
converge, sheaf_validate, commit) with timing, energy snapshots, and attempt counts.
These records are surfaced by ``perspt status``, the dashboard decisions page, and the
headless agent summary.

The policy is auto-selected based on workspace state (greenfield vs existing project).
``needs_architect()`` gates whether the Architect tier runs; ``needs_speculator()``
gates the speculator lookahead call.


Feature Charter
~~~~~~~~~~~~~~~

Before architect planning begins, the orchestrator creates a ``FeatureCharter``
with policy-derived defaults:

- **LocalEdit**: max 1 module, 5 files, 3 revisions
- **FeatureIncrement**: max 10 modules, 30 files, 5 revisions
- **LargeFeature / GreenfieldBuild / ArchitecturalRevision**: max 25 modules, 80 files, 10 revisions

The charter gates the plan: if the Architect produces a plan exceeding the charter's
module or file budget, a warning is emitted. Language constraints are derived from
active plugins.


Retry Policy
------------

SRBN implements bounded retries per error type:

.. list-table::
   :header-rows: 1
   :widths: 30 20 50

   * - Error Type
     - Max Retries
     - Escalation
   * - Compilation errors (LSP)
     - 3
     - Escalate to user with diagnostic context
   * - Tool failures (file ops)
     - 5
     - Escalate with error logs
   * - Review rejections (user)
     - 3
     - Escalate with diff summary

When retries are exhausted, the node transitions to **Escalated** state.
Escalated nodes do not block subsequent nodes. The orchestrator tracks
completed and escalated counts and derives the final ``SessionOutcome``:
``Success`` if all nodes completed, ``PartialSuccess`` if some escalated,
or ``Failed`` if none completed. In headless mode (``--yes``), escalations
are logged and the session exits with a non-zero code when the outcome
is not ``Success``.


Artifact Bundle Protocol
------------------------

The Actuator emits a JSON artifact bundle for each node:

.. code-block:: json

   {
     "artifacts": [
       {
         "path": "src/lib.rs",
         "operation": "write",
         "content": "pub fn add(a: i32, b: i32) -> i32 { a + b }"
       },
       {
         "path": "src/main.rs",
         "operation": "diff",
         "patch": "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1,3 @@..."
       }
     ],
     "commands": [
       "cargo build"
     ]
   }

Operations:

- **write** — Create or overwrite a file with the given content
- **diff** — Apply a unified diff patch to an existing file
- **command** — Execute a shell command (validated by policy engine)

All artifacts are applied transactionally. If any operation fails, the entire
bundle is rolled back.


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

- **Integrity** — Each commit has a cryptographic hash chaining to its parent
- **Rollback** — Revert to any previous state via ``perspt ledger --rollback``
- **Resume** — ``perspt resume`` rehydrates session state including energy history,
  retry counts, and escalation reports
- **Audit** — Complete trail of AI-generated changes with energy breakdowns

.. code-block:: bash

   perspt ledger --recent     # View recent commits
   perspt ledger --rollback abc123
   perspt ledger --stats      # Session statistics


Provisional Branches
--------------------

When SRBN speculates on child nodes before the parent is fully committed, it uses
**provisional branches** to isolate speculative work.

.. admonition:: Key Invariant
   :class: important

   Provisional work is **never** merged into the global ledger until the parent node
   meets the stability threshold. If the parent fails, all dependent branches are
   flushed.

Branch Lifecycle
~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - State
     - Description
   * - **Active**
     - Branch is open, speculative work in progress
   * - **Sealed**
     - Parent interface is sealed; children may proceed
   * - **Merged**
     - Parent committed; branch work merged into global ledger
   * - **Flushed**
     - Parent failed; branch work discarded (may be replayed later)

Interface Seals
~~~~~~~~~~~~~~~

Interface nodes produce a structural digest (SHA-256 hash) of their public API
after reaching the Commit phase. This seal is injected into child node restriction
maps, ensuring children code against stable signatures.

1. Parent node reaches **Commit** phase
2. If the node is an **Interface** class, its exported signatures are hashed
3. ``InterfaceSealed`` event is emitted with sealed paths and hash
4. Blocked dependents are released
5. Seal digests are available to child verifiers for contract checking

Flush Cascade
~~~~~~~~~~~~~

When a parent node fails verification:

1. The parent's provisional branch is flushed
2. ``collect_descendants`` walks the DAG to find all transitive children
3. Each descendant branch is flushed recursively
4. ``BranchFlushed`` event is emitted with the reason and affected IDs
