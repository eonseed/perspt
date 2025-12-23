.. _user-guide-agent-mode:

Agent Mode
==========

Autonomous code generation with SRBN.

What is Agent Mode?
-------------------

Agent Mode uses the **Stabilized Recursive Barrier Network (SRBN)** to autonomously:

1. Decompose tasks into subtasks
2. Generate code for each subtask
3. Verify with LSP and tests
4. Commit stable changes to the Merkle ledger

Quick Start
-----------

.. code-block:: bash

   perspt agent "Create a Python calculator"

How It Works
------------

.. graphviz::
   :align: center

   digraph srbn {
       rankdir=LR;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10];
       
       task [label="Task", shape=ellipse, fillcolor="#E3F2FD"];
       sheaf [label="Sheafify", fillcolor="#E8F5E9"];
       spec [label="Speculate", fillcolor="#FFF3E0"];
       verify [label="Verify", fillcolor="#F3E5F5"];
       commit [label="Commit", fillcolor="#C8E6C9"];
       
       task -> sheaf -> spec -> verify;
       verify -> spec [label="retry", style=dashed];
       verify -> commit [label="stable"];
   }

Model Tiers
-----------

.. list-table::
   :header-rows: 1
   :widths: 20 30 50

   * - Tier
     - Purpose
     - Example
   * - Architect
     - Task decomposition
     - ``--architect-model gpt-5.2``
   * - Actuator
     - Code generation
     - ``--actuator-model claude-opus-4.5``
   * - Verifier
     - Stability check
     - ``--verifier-model gemini-3-pro``
   * - Speculator
     - Fast lookahead
     - ``--speculator-model gemini-3-flash``

Common Commands
---------------

.. code-block:: bash

   # Basic
   perspt agent "Create module"

   # With workspace
   perspt agent -w ./project "Add tests"

   # Auto-approve
   perspt agent -y "Refactor"

   # Production-grade
   perspt agent \
     --architect-model gpt-5.2 \
     --stability-threshold 0.05 \
     --max-cost 10.0 \
     "Implement auth"

Review Process
--------------

When changes need approval:

.. code-block:: text

   ╭─────────────────────────────╮
   │  Review Changes             │
   ╞═════════════════════════════╡
   │  + main.py (new)           │
   │  + tests/test_main.py (new)│
   │                             │
   │  [y] Approve  [n] Reject   │
   ╰─────────────────────────────╯

Session Control
---------------

.. code-block:: bash

   perspt status  # Check progress
   perspt abort   # Cancel
   perspt resume  # Resume

See Also
--------

- :doc:`../tutorials/agent-mode` - Full tutorial
- :doc:`../concepts/srbn-architecture` - Technical details
- :doc:`../howto/agent-options` - CLI reference
