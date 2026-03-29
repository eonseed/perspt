.. _howto-agent-options:

Agent Options Reference
=======================

Complete reference for ``perspt agent`` flags.

Basic Options
-------------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Flag
     - Description
   * - ``<TASK>``
     - Task description (positional argument)
   * - ``-w, --workdir <DIR>``
     - Working directory for code generation
   * - ``-y, --yes``
     - Auto-approve all changes (headless mode)
   * - ``--model <MODEL>``
     - LLM model for all tiers
   * - ``--single-file``
     - Single-file mode (no DAG planning)

Per-Tier Model Selection
------------------------

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Flag
     - Description
   * - ``--architect-model <M>``
     - Model for task decomposition and planning
   * - ``--actuator-model <M>``
     - Model for code generation
   * - ``--verifier-model <M>``
     - Model for stability analysis
   * - ``--speculator-model <M>``
     - Model for branch prediction
   * - ``--architect-fallback-model <M>``
     - Fallback for architect tier
   * - ``--actuator-fallback-model <M>``
     - Fallback for actuator tier
   * - ``--verifier-fallback-model <M>``
     - Fallback for verifier tier
   * - ``--speculator-fallback-model <M>``
     - Fallback for speculator tier

Energy and Convergence
----------------------

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Flag
     - Description
   * - ``--energy-weights <W>``
     - Comma-separated ``alpha,beta,gamma`` (default: ``1.0,0.5,1.0``)
   * - ``--stability-threshold <E>``
     - Convergence epsilon (default: ``0.10``)
   * - ``--verifier-strictness <S>``
     - ``default``, ``strict``, or ``minimal``

Cost and Limits
---------------

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Flag
     - Description
   * - ``--max-cost <USD>``
     - Maximum total LLM spend
   * - ``--max-steps <N>``
     - Maximum total iterations across all nodes

Execution Control
-----------------

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Flag
     - Description
   * - ``--mode <MODE>``
     - ``cautious``, ``balanced``, or ``yolo``
   * - ``-k <N>``
     - Complexity threshold for balanced mode
   * - ``--complexity <LEVEL>``
     - ``low``, ``medium``, ``high``, ``critical``
   * - ``--defer-tests``
     - Skip V_log during node coding
   * - ``--auto-approve-safe``
     - Auto-approve nodes below complexity threshold

Logging and Output
------------------

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Flag
     - Description
   * - ``--log-llm``
     - Log all LLM calls to DuckDB
   * - ``--output-plan <FILE>``
     - Export task plan as JSON before execution

Examples
--------

.. code-block:: bash

   # Simple headless run
   perspt agent -y -w /tmp/proj "Create a Python calculator"

   # Full control
   perspt agent \
     --architect-model gemini-pro-latest \
     --actuator-model gemini-3.1-flash-lite-preview \
     --verifier-model gemini-pro-latest \
     --energy-weights "1.0,1.0,2.0" \
     --stability-threshold 0.05 \
     --max-cost 5.0 \
     --max-steps 30 \
     --log-llm \
     --mode balanced \
     -w ./project "Build a web server"
