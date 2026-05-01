.. _psp-process:

PSP Process
===========

**Perspt Specification Proposals (PSPs)** are design documents that describe
significant features, architectural changes, or process improvements.

Purpose
-------

PSPs serve as the primary mechanism for:

- Proposing major features before implementation
- Documenting design decisions and trade-offs
- Providing a historical record of architectural evolution
- Enabling community review of significant changes

PSP Lifecycle
-------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Status
     - Meaning
   * - **Draft**
     - Proposal is being written
   * - **Active**
     - Proposal is accepted and under implementation
   * - **Final**
     - Implementation is complete
   * - **Superseded**
     - Replaced by a newer PSP

Key PSPs
--------

.. list-table::
   :header-rows: 1
   :widths: 15 40 45

   * - PSP
     - Title
     - Status
   * - PSP-1
     - Core Chat Interface
     - Final
   * - PSP-2
     - Multi-Provider Support
     - Final
   * - PSP-3
     - Simple CLI Mode
     - Final
   * - PSP-4
     - SRBN Agent Mode
     - Superseded by PSP-5
   * - **PSP-5**
     - **Multi-File Coding UX and Repo-Native Verification**
     - **Final**
   * - PSP-6
     - Web Dashboard and Real-Time Monitoring
     - Final
   * - **PSP-7**
     - **Robust Typed Correction Loops and Plugin-Aware Prompt Contracts**
     - **Active**

PSP-5: The Core Lifecycle
~~~~~~~~~~~~~~~~~~~~~~~~~

PSP-5 is the operative specification that governs the current SRBN agent runtime. It
supersedes PSP-4 and introduces:

- **Project-first execution** — multi-file DAGs instead of single-file tasks
- **Ownership closure** — each file owned by exactly one node
- **Artifact bundle protocol** — structured JSON with write/diff/command operations
- **Node classes** — Interface, Implementation, Integration
- **Five-component energy** — V_syn, V_str, V_log, V_boot, V_sheaf
- **Plugin-driven verification** — language plugins select LSP, test runner, init commands
- **Provisional branches** — speculative child execution isolated until parent commits
- **Interface seals** — SHA-256 digest of exported signatures for dependency management
- **Deterministic fallback planner** — handles plan parsing failures gracefully
- **Ledger-based resume** — trustworthy session continuation from any interruption point

See the full specification at ``docs/psps/source/psp-000005.rst``.

PSP-7: The Current Runtime
~~~~~~~~~~~~~~~~~~~~~~~~~~

PSP-7 is the operative specification that governs the current SRBN agent runtime. It
builds on PSP-5's foundation and introduces:

- **Typed parse pipeline** — 5-layer fail-closed parsing (raw capture → strict JSON → tolerant recovery → schema validation → semantic filtering) replacing Option-based extraction
- **Retry classification** — ``RetryClassification`` enum (MalformedRetry, Retarget, SupportFileViolation, Replan) enabling intelligent convergence loop decisions
- **Prompt compiler with provenance** — Structured ``PromptEvidence`` replaces ad-hoc template constants; exact target paths from evidence included in correction prompts
- **Structured JSON artifact format** — ``{ artifacts: [], commands: [] }`` schema replaces free-form ``File: ...`` output instructions
- **Manifest policy enforcement** — Semantic validation prevents implicit mutation of root manifests unless explicitly listed as output targets
- **Strict budget exhaustion** — ``any_exhausted()`` checks all budget dimensions (steps, revisions, cost) before LLM calls
- **Correction attempt records** — Every correction attempt persisted with parse state, retry classification, energy snapshot, and response fingerprint for full observability

PSP-5 remains the foundation for the core SRBN lifecycle. See the full specification at
``docs/psps/source/psp-000005.rst``.

See the PSP-7 specification at ``docs/psps/source/psp-000007.rst``.

Writing a PSP
-------------

1. Fork the repository and create a branch
2. Copy the PSP template from ``docs/psps/source/``
3. Fill in the sections: Abstract, Motivation, Specification, Rationale, Reference
4. Submit a pull request for community review
5. Iterate based on feedback
