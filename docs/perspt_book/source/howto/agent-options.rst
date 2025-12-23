.. _howto-agent-options:

Agent Options
=============

Complete reference for SRBN agent configuration.

Basic Usage
-----------

.. code-block:: bash

   perspt agent [OPTIONS] <TASK>

Required Arguments
------------------

.. list-table::
   :widths: 20 80

   * - ``<TASK>``
     - Task description or path to task file

Model Selection
---------------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Option
     - Description
   * - ``--model <MODEL>``
     - Override ALL model tiers
   * - ``--architect-model <MODEL>``
     - Model for task decomposition (deep reasoning)
   * - ``--actuator-model <MODEL>``
     - Model for code generation
   * - ``--verifier-model <MODEL>``
     - Model for stability checking
   * - ``--speculator-model <MODEL>``
     - Model for fast lookahead

**Example**:

.. code-block:: bash

   perspt agent \
     --architect-model gpt-5.2 \
     --actuator-model claude-opus-4.5 \
     "Build REST API"

Execution Control
-----------------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Option
     - Description
   * - ``-w, --workdir <DIR>``
     - Working directory (default: current)
   * - ``-y, --yes``
     - Auto-approve all actions
   * - ``--auto-approve-safe``
     - Auto-approve read-only operations only
   * - ``-k, --complexity <K>``
     - Max tasks before approval prompt (default: 5)
   * - ``--mode <MODE>``
     - Execution mode: ``cautious``, ``balanced``, ``yolo``

**Modes**:

.. list-table::
   :widths: 20 80

   * - ``cautious``
     - Prompt for every change
   * - ``balanced``
     - Prompt when complexity > K (default)
   * - ``yolo``
     - Auto-approve everything (⚠️ dangerous)

SRBN Parameters
---------------

.. list-table::
   :header-rows: 1
   :widths: 40 60

   * - Option
     - Description
   * - ``--energy-weights <α,β,γ>``
     - Lyapunov weights (default: ``1.0,0.5,2.0``)
   * - ``--stability-threshold <ε>``
     - Convergence threshold (default: ``0.1``)

**Energy Formula**: V(x) = α·V_syn + β·V_str + γ·V_log

**Tuning Examples**:

.. code-block:: bash

   # Prioritize tests (raise γ)
   perspt agent --energy-weights "1.0,0.5,3.0" "Add tests"

   # Prioritize type safety (raise α)
   perspt agent --energy-weights "2.0,0.5,1.0" "Add type hints"

   # More lenient (raise ε)
   perspt agent --stability-threshold 0.5 "Quick prototype"

Limits
------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Option
     - Description
   * - ``--max-cost <USD>``
     - Maximum cost in dollars (0 = unlimited)
   * - ``--max-steps <N>``
     - Maximum iterations (0 = unlimited)

**Example**:

.. code-block:: bash

   perspt agent --max-cost 5.0 --max-steps 20 "Large refactor"

Session Management
------------------

.. code-block:: bash

   # Check current status
   perspt status

   # Cancel current session
   perspt abort
   perspt abort --force  # No confirmation

   # Resume interrupted session
   perspt resume
   perspt resume <session_id>

Ledger Operations
-----------------

.. code-block:: bash

   # View recent changes
   perspt ledger --recent

   # Rollback to commit
   perspt ledger --rollback <hash>

   # Statistics
   perspt ledger --stats

Full Examples
-------------

**Conservative approach**:

.. code-block:: bash

   perspt agent \
     --mode cautious \
     -k 1 \
     --max-cost 1.0 \
     --max-steps 10 \
     -w ./project \
     "Add input validation"

**Fast prototyping**:

.. code-block:: bash

   perspt agent -y \
     --model gemini-3-flash \
     --stability-threshold 0.5 \
     "Create boilerplate"

**Production-grade**:

.. code-block:: bash

   perspt agent \
     --architect-model gpt-5.2 \
     --actuator-model claude-opus-4.5 \
     --verifier-model gemini-3-pro \
     --energy-weights "2.0,1.0,3.0" \
     --stability-threshold 0.05 \
     --max-cost 10.0 \
     -w ./project \
     "Implement authentication system"

See Also
--------

- :doc:`../concepts/srbn-architecture` - SRBN details
- :doc:`../tutorials/agent-mode` - Tutorial
- :doc:`configuration` - Config file
