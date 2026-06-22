.. _psp-process:

PSP Process
===========

A complex software system cannot be modified by arbitrary whim. We must frame every major modification as a formal proposal, detailing the design, the motivation, and the proof of correctness. We call such a proposal a **Perspt Specification Proposal (PSP)**.

PSPs serve as the primary mechanism for:

- Proposing major features before implementation.
- Documenting design decisions and trade-offs.
- Providing a historical record of architectural evolution.
- Enabling community review of significant changes.

PSP Lifecycle
-------------

A proposal passes through four distinct states. First, it is written as a **Draft**. Once accepted for implementation, it becomes **Active**. When the implementation is complete and verified, the proposal becomes **Final**. If a subsequent proposal modifies the same system, the earlier proposal may become **Superseded**.

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Status
     - Meaning
   * - **Draft**
     - Proposal is under active design and review.
   * - **Active**
     - Proposal is accepted and under active implementation.
   * - **Final**
     - Implementation is complete and fully verified.
   * - **Superseded**
     - Replaced or invalidated by a newer specification proposal.

Key PSPs
--------

We tabulate the specifications that govern the architecture of the system.

.. list-table::
   :header-rows: 1
   :widths: 15 45 40

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
     - **Final**
   * - **PSP-8**
     - **SRBN Agent SDK, Coding Domain Package, and Mutable Work Graph**
     - **Draft**

PSP-5: The Core Lifecycle
~~~~~~~~~~~~~~~~~~~~~~~~~

PSP-5 is the operative specification that governs the core multi-file agent execution. It supersedes PSP-4 and establishes:

- **Project-first execution** - The task is modeled as a directed acyclic graph (DAG) of nodes instead of a single-file task.
- **Ownership closure** - Each file is owned by exactly one node.
- **Artifact bundle protocol** - Structured operations containing writes, diffs, and commands.
- **Node classes** - Division of nodes into Interface, Implementation, and Integration classes.
- **Five-component energy** - The Lyapunov energy model decomposed into syntactic, structural, logical, bootstrap, and sheaf components.
- **Plugin-driven verification** - Selection of language-specific verification tools.
- **Provisional branches** - Speculative execution isolated from the main workspace until parent nodes commit.
- **Interface seals** - SHA-256 digests of exported signatures to enforce cross-node contracts.

See the full specification at ``docs/psps/source/psp-000005.rst``.

PSP-7: The Correction Loop
~~~~~~~~~~~~~~~~~~~~~~~~~~

PSP-7 builds upon the foundation of PSP-5 to enforce robust convergence under non-deterministic failures. It introduces:

- **Typed parse pipeline** - A five-layer fail-closed parsing protocol that recovers from malformed model outputs.
- **Retry classification** - Explicit categorization of verification failures to determine whether to retarget, replan, or retry.
- **Prompt compiler with provenance** - Systematic generation of correction prompts using structural error evidence.
- **Manifest policy enforcement** - Protection of root manifests from implicit mutation.
- **Strict budget exhaustion** - Interception of execution before LLM invocation when steps, revisions, or cost limits are exceeded.

See the full specification at ``docs/psps/source/psp-000007.rst``.

PSP-8: The Reusable Platform
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

PSP-8 defines the architecture for separating domain-neutral control mechanisms from domain-specific features. It introduces:

- **SDK-First design** - Movement of scheduling, energy gating, capability checks, and replay ledgers to a reusable SDK.
- **Quadratic energy** - Adoption of the quadratic residual energy formula :math:`V(x) = \sum_{e} w_e \lVert r_e \rVert^2`.
- **Mutable work graph** - A scheduler that modifies, splits, or inserts nodes in the queue as verification evidence arrives.
- **Capability kernel** - Scoped admissibility check over proposed effects to guarantee sandbox bounds.

See the full specification at ``docs/psps/source/psp-000008.rst``.

Writing a PSP
-------------

A developer wishing to propose an architectural modification must adhere to the following protocol:

1. Create a branch from the repository root.
2. Instantiate a new document in ``docs/psps/source/`` following the standard template.
3. Define the Abstract, Motivation, Specification, Rationale, and Reference Implementation.
4. Submit the proposal for community review.
5. Merge the document when consensus is reached and status is updated.
