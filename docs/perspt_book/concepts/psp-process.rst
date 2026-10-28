.. _psp-process:

PSP Process
===========

**A Perspt Specification Proposal (PSP) records a significant design change and its review.** Routine fixes use normal pull requests.
The catalog preserves both implemented designs and proposals under discussion.
A proposal's lifecycle state and its implementation coverage are separate facts.

Find the current state
----------------------

The `PSP registry <../../psps/registry.html>`_ is generated from record metadata.
It is the source for current proposal numbers, titles, and states.
We avoid duplicating that registry in the book.
The `catalog workflow <../../psps/workflow.html>`_ documents the allowed transitions.

The configured states are ``unassigned``, ``draft``, ``discussion``, ``accepted``, ``active``, ``final``, ``rejected``, and ``withdrawn``.
Maintainers assign a number with ``guidedog dsc promote``; the proposal remains ``draft``.
They open discussion separately with ``guidedog dsc state`` and an explicit reason.
Merging a pull request does not number a proposal or change its state.
``active`` denotes a living specification; ``withdrawn`` includes superseded proposals.

.. code-block:: bash

   guidedog dsc list --dir=docs/psps
   guidedog dsc check --dir=docs/psps

Read the design history
-----------------------

The summaries below describe the ideas introduced by each proposal.
For current behavior, use :doc:`../user-guide/agent-mode` and :doc:`srbn-architecture`.

PSP-5: The Core Lifecycle
~~~~~~~~~~~~~~~~~~~~~~~~~

PSP-5 is the historical specification of the original multi-file agent execution; the operative platform is now defined by PSP-9 and PSP-10. It supersedes PSP-4 and established:

- **Project-first execution** - The task is modeled as a directed acyclic graph (DAG) of nodes instead of a single-file task.
- **Ownership closure** - Each file is owned by exactly one node.
- **Artifact bundle protocol** - Structured operations containing writes, diffs, and commands.
- **Node classes** - Division of nodes into Interface, Implementation, and Integration classes.
- **Five-component energy** - The Lyapunov energy model decomposed into syntactic, structural, logical, bootstrap, and sheaf components.
- **Plugin-driven verification** - Selection of language-specific verification tools.
- **Provisional branches** - Speculative execution isolated from the main workspace until parent nodes commit.
- **Interface seals** - SHA-256 digests of exported signatures to enforce cross-node contracts.

See the full specification at ``docs/psps/records/000005-srbn-multi-file-coding-ux.rst``.

PSP-7: The Correction Loop
~~~~~~~~~~~~~~~~~~~~~~~~~~

PSP-7 builds upon the foundation of PSP-5 to enforce robust convergence under non-deterministic failures. It introduces:

- **Typed parse pipeline** - A five-layer fail-closed parsing protocol that recovers from malformed model outputs.
- **Retry classification** - Explicit categorization of verification failures to determine whether to retarget, replan, or retry.
- **Prompt compiler with provenance** - Systematic generation of correction prompts using structural error evidence.
- **Manifest policy enforcement** - Protection of root manifests from implicit mutation.
- **Strict budget exhaustion** - Interception of execution before LLM invocation when steps, revisions, or cost limits are exceeded.

See the full specification at ``docs/psps/records/000007-robust-typed-correction-loops.rst``.

PSP-8: The Reusable Platform
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

PSP-8 defines the architecture for separating domain-neutral control mechanisms from domain-specific features. It introduces:

- **SDK-First design** - Movement of scheduling, energy gating, capability checks, and replay ledgers to a reusable SDK.
- **Quadratic energy** - Adoption of the quadratic residual energy formula :math:`V(x) = \sum_{e} w_e \lVert r_e \rVert^2`.
- **Mutable work graph** - A scheduler that modifies, splits, or inserts nodes in the queue as verification evidence arrives.
- **Capability kernel** - Scoped admissibility checks over proposed effects.

See the full specification at ``docs/psps/records/000008-srbn-agent-sdk-work-graph.rst``.

PSP-9: The Governed Platform
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

PSP-9 replaces bundle generation with a governed tool loop and defines the operative agent runtime. It introduces:

- **Governed tool loop** - Every model-issued tool call becomes a typed proposal; a deterministic admissibility kernel decides whether it may affect the candidate.
- **Measured acceptance gate** - A candidate is admitted on a hard verifier pass or a measured energy descent of at least :math:`\rho_{\text{gate}}` below the best accepted state.
- **Model portfolio** - Per-role routes (actuator, explorer, adjudicator) with an ordered sticky fallback chain, configured via ``[models]`` or CLI flags.
- **Durable event ledger** - A hash-chained event stream enabling deterministic, credential-free replay and crash resume.
- **Open language registry** - A ``LanguageId``-keyed adapter registry replaces closed per-language enum dispatch.

See the full specification at ``docs/psps/records/000009-governed-agentic-platform.rst``.

PSP-10: Search and Prompt Programs
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

PSP-10 extends the governed platform with bounded search trajectories and model-conditioned prompt programs. It introduces:

- **Typed prompt sections** - Prompt libraries compiled at build time from versioned section files, with program and invocation digests recorded in the ledger.
- **Bounded search forests** - On gate failure, isolated branches run against the accepted root under pre-reserved budgets, with exact no-good learning.
- **Resident-context paging** - The conversation becomes content-addressed pages; evicted pages leave tombstones and remain recallable via ``context_recall``.
- **Multi-node dispatch** - Concurrent work-graph nodes behind ``--max-parallel-nodes``, staged into a content-addressed root and promoted through one global integration gate.

See the full specification at ``docs/psps/records/000010-bounded-search-trajectories.rst``.

PSP-11: Domain-Neutral Runtime Inversion & Multi-Actor Supervisor
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

PSP-11 specifies the inversion of the governed candidate engine into ``perspt-sdk``, a multi-actor supervisor over shared durable stores, and replayable human interaction gates.

See the full specification at ``docs/psps/records/000011-domain-neutral-runtime-inversion.rst``.

Writing a PSP
-------------

A developer wishing to propose an architectural modification must adhere to the following protocol:

1. Create a branch from the repository root.
2. Run ``guidedog dsc new "Proposal title" --dir=docs/psps`` to create an unnumbered record in ``docs/psps/records/``.
3. Define the Abstract, Motivation, Specification, Rationale, and Reference Implementation.
4. Run ``guidedog dsc index --dir=docs/psps && guidedog dsc check --dir=docs/psps`` and submit the proposal for review.
5. Maintainers assign a number and change the record's state through reviewed pull requests, using ``guidedog dsc promote`` and ``guidedog dsc state``.
