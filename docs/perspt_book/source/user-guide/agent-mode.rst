.. _user-guide-agent-mode:

Agent Mode
==========

The agent command activates the experimental SRBN (Stabilized Recursive Barrier Network)
engine for autonomous multi-file code generation.

Launching Agent Mode
--------------------

.. code-block:: bash

   perspt agent -w <DIR> "<task>"

   # Examples
   perspt agent -w ./my-project "Create a Python REST API"
   perspt agent -y -w /tmp/demo "Build a Rust CLI tool"

Core Workflow
-------------

The SRBN agent follows the PSP-5 lifecycle:

1. **Detection** — Identify workspace state (greenfield/brownfield) and select
   language plugins
2. **Planning** — Architect model decomposes the task into a DAG of nodes with
   assigned classes (Interface, Implementation, Integration)
3. **Execution** — For each node in topological order:

   a. Actuator generates a multi-artifact bundle (writes, diffs, commands)
   b. Bundle is applied transactionally
   c. Verification computes Lyapunov energy: V(x) = α·V_syn + β·V_str + γ·V_log + V_boot + V_sheaf
   d. If V(x) < ε (default 0.10), node is stable; otherwise retry

4. **Sheaf Validation** — Cross-node contract verification
5. **Review** — In interactive mode: grouped-diff modal with approve/reject/correct
6. **Commit** — Stable nodes are committed to the Merkle ledger

Node Classes
------------

Each DAG node belongs to a class that governs scheduling and verification:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Class
     - Description
   * - **Interface**
     - Public API definitions, type signatures, traits. Scheduled first.
   * - **Implementation**
     - Internal logic. May depend on Interface nodes.
   * - **Integration**
     - Wiring, main entry, config assembly. Scheduled last.

Artifact Bundle Protocol
------------------------

Each node produces a bundle with three artifact types:

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Type
     - Description
   * - ``write``
     - Create a new file with full content
   * - ``diff``
     - Modify an existing file (unified diff format)
   * - ``command``
     - Execute a shell command (e.g., ``uv add pandas``)

Ownership closure ensures no two nodes own the same file.

Review Modal (Interactive)
--------------------------

When running without ``--yes``, the review modal presents:

.. code-block:: text

   Review Node 3: Implement data transformer
   ──────────────────────────────────────────
   Bundle: 1 created, 1 modified
   + src/transformer.py    [create] (45 lines)
   ~ src/pipeline.py       [diff]   (+3, -1)

   Verification: V_syn OK | V_str OK | V_log OK | V_boot OK
   Energy: V(x) = 0.00

   [y] Approve  [n] Reject  [c] Correct  [e] Edit  [d] Diff

- **y** — Approve and commit to ledger
- **n** — Reject and regenerate
- **c** — Send feedback for correction
- **e** — Open files in your editor
- **d** — Toggle full diff view

Session Management
------------------

.. code-block:: bash

   # Check session state (per-node counts, energy, escalations)
   perspt status

   # Abort the current session
   perspt abort

   # Resume with trust context (shows escalation count, energy, retries)
   perspt resume --last

See :doc:`advanced-features` for model tiers, energy tuning, and cost controls.
