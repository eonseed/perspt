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
     - **Active**

PSP-5: The Current Runtime
~~~~~~~~~~~~~~~~~~~~~~~~~~

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

Writing a PSP
-------------

1. Fork the repository and create a branch
2. Copy the PSP template from ``docs/psps/source/``
3. Fill in the sections: Abstract, Motivation, Specification, Rationale, Reference
4. Submit a pull request for community review
5. Iterate based on feedback
