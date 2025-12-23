.. _tutorial-agent-mode:

Agent Mode Tutorial
===================

Master autonomous code generation with SRBN.

Overview
--------

Agent Mode lets Perspt autonomously write, test, and verify code using the 
**Stabilized Recursive Barrier Network (SRBN)** engine.

Prerequisites
-------------

- Perspt v0.5.0+ installed
- API key for a capable model (GPT-5.2, Claude Opus 4.5 recommended)
- Python 3.9+ (for LSP integration)

Basic Usage
-----------

.. code-block:: bash

   # Simple task
   perspt agent "Create a Python calculator"

   # With workspace
   perspt agent -w ./my-project "Add unit tests"

   # Auto-approve all
   perspt agent -y "Refactor error handling"

Step-by-Step Example
--------------------

Let's create a Python calculator:

Step 1: Start the Agent
~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   mkdir calculator-demo && cd calculator-demo
   perspt agent "Create a Python calculator with add, subtract, multiply, divide operations. Include type hints and a comprehensive test suite."

Step 2: Watch the SRBN Loop
~~~~~~~~~~~~~~~~~~~~~~~~~~~

The agent will:

1. **Sheafify**: Decompose into subtasks

   .. code-block:: json

      {
        "nodes": [
          {"id": 1, "description": "Create Calculator class"},
          {"id": 2, "description": "Add arithmetic methods"},
          {"id": 3, "description": "Write unit tests"}
        ]
      }

2. **Speculate**: Generate code for each node

3. **Verify**: Check with LSP and tests

   .. code-block:: text

      V(x) = 1.0·V_syn + 0.5·V_str + 2.0·V_log
      V_syn = 0 (no LSP errors)
      V_str = 0.1 (clean structure)
      V_log = 0 (all tests pass)
      V(x) = 0.05 < ε (stable!)

4. **Commit**: Record in ledger

Step 3: Review Changes
~~~~~~~~~~~~~~~~~~~~~~

When prompted, review the generated code:

.. code-block:: text

   ╭─────────────────────────────────────────────────────╮
   │  Review Changes                                     │
   ╞═════════════════════════════════════════════════════╡
   │  + calculator.py   (new file, 45 lines)            │
   │  + test_calculator.py (new file, 62 lines)         │
   │                                                     │
   │  [y] Approve  [n] Reject  [d] View Diff            │
   ╰─────────────────────────────────────────────────────╯

Step 4: Check Results
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # View generated files
   ls -la
   # calculator.py
   # test_calculator.py

   # Run tests
   python -m pytest test_calculator.py -v

Model Tier Configuration
------------------------

Use specialized models for different SRBN phases:

.. code-block:: bash

   perspt agent \
     --architect-model gpt-5.2 \
     --actuator-model claude-opus-4.5 \
     --verifier-model gemini-3-pro \
     --speculator-model gemini-3-flash \
     "Build a REST API"

.. list-table::
   :header-rows: 1
   :widths: 20 40 40

   * - Tier
     - Purpose
     - Recommendation
   * - Architect
     - Task decomposition
     - Deep reasoning (GPT-5.2)
   * - Actuator
     - Code generation
     - Strong coding (Claude)
   * - Verifier
     - Stability check
     - Fast analysis (Gemini Pro)
   * - Speculator
     - Branch prediction
     - Ultra-fast (Gemini Flash)

Energy Tuning
-------------

Customize the Lyapunov energy weights:

.. code-block:: bash

   # Prioritize test passing (higher γ)
   perspt agent --energy-weights "1.0,0.5,3.0" "Add tests"

   # Prioritize type safety (higher α)
   perspt agent --energy-weights "2.0,0.5,1.0" "Add type hints"

Execution Modes
---------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Mode
     - Behavior
   * - ``cautious``
     - Prompt for approval on every change
   * - ``balanced``
     - Prompt when complexity > K (default)
   * - ``yolo``
     - Auto-approve everything (dangerous!)

.. code-block:: bash

   perspt agent --mode cautious "Modify database schema"

Complexity Threshold
--------------------

Control when to prompt for approval:

.. code-block:: bash

   # Approve up to 3 files without prompting
   perspt agent -k 3 "Refactor module"

   # Always prompt (k=0)
   perspt agent -k 0 "Any task"

Cost and Step Limits
--------------------

.. code-block:: bash

   # Maximum $5 cost
   perspt agent --max-cost 5.0 "Large refactor"

   # Maximum 10 iterations
   perspt agent --max-steps 10 "Iterative improvement"

Managing Sessions
-----------------

.. code-block:: bash

   # Check status
   perspt status

   # Abort current
   perspt abort

   # Resume interrupted
   perspt resume

Change Tracking
---------------

.. code-block:: bash

   # View history
   perspt ledger --recent

   # Rollback
   perspt ledger --rollback abc123

   # Statistics
   perspt ledger --stats

Best Practices
--------------

1. **Start small**: Test with simple tasks first
2. **Use workspace**: Always specify ``-w`` for clarity
3. **Set limits**: Use ``--max-cost`` and ``--max-steps``
4. **Review carefully**: Check diffs before approving
5. **Use tiers**: Match models to task requirements
6. **Track changes**: Use ``perspt ledger`` regularly

Troubleshooting
---------------

**Agent stuck in retry loop**:

- Check LSP is working: ``ty check file.py``
- Lower stability threshold: ``--stability-threshold 0.5``
- Reduce energy weights for less strict verification

**High energy despite clean code**:

- Check test failures: ``pytest -v``
- Review LSP diagnostics
- Adjust weights: ``--energy-weights "0.5,0.5,1.0"``

See Also
--------

- :doc:`../concepts/srbn-architecture` - SRBN details
- :doc:`../howto/agent-options` - Full CLI reference
- :doc:`../api/perspt-agent` - API documentation
