.. _user-guide-advanced-features:

Advanced Features
=================

Power user capabilities in Perspt.

Model Tier Configuration
------------------------

Use specialized models for each SRBN phase:

.. code-block:: bash

   perspt agent \
     --architect-model gpt-5.2 \
     --actuator-model claude-opus-4.5 \
     --verifier-model gemini-3-pro \
     --speculator-model gemini-3-flash \
     "Build API"

Energy Tuning
-------------

Customize Lyapunov energy weights:

.. math::

   V(x) = \alpha \cdot V_{syn} + \beta \cdot V_{str} + \gamma \cdot V_{log}

.. code-block:: bash

   # Prioritize tests (raise γ)
   perspt agent --energy-weights "1.0,0.5,3.0" "Add tests"

   # Prioritize type safety (raise α)
   perspt agent --energy-weights "2.0,0.5,1.0" "Add type hints"

Stability Threshold
-------------------

Control convergence sensitivity:

.. code-block:: bash

   # Stricter (production)
   perspt agent --stability-threshold 0.05 "Critical fix"

   # Lenient (prototyping)
   perspt agent --stability-threshold 0.5 "Quick draft"

Execution Modes
---------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Mode
     - Behavior
   * - ``cautious``
     - Prompt for every change
   * - ``balanced``
     - Prompt when complexity > K (default)
   * - ``yolo``
     - Auto-approve everything (⚠️ dangerous)

Cost and Step Limits
--------------------

.. code-block:: bash

   # Set budget
   perspt agent --max-cost 5.0 "Large refactor"

   # Limit iterations
   perspt agent --max-steps 10 "Iterative task"

Merkle Ledger
-------------

Track and rollback changes:

.. code-block:: bash

   # View history
   perspt ledger --recent

   # Rollback
   perspt ledger --rollback abc123

   # Statistics
   perspt ledger --stats

Policy Rules
------------

Create custom Starlark rules:

.. code-block:: python

   # .perspt/rules.star
   allow("cat *")
   prompt("rm *", reason="File deletion")
   deny("rm -rf /")

Project Memory
--------------

Use ``PERSPT.md`` for project context:

.. code-block:: markdown

   # My Project

   ## Tech Stack
   - Python 3.11
   - FastAPI
   - PostgreSQL

   ## Conventions
   - Type hints everywhere
   - 100% test coverage

Session Management
------------------

.. code-block:: bash

   # Check status
   perspt status

   # Abort
   perspt abort

   # Resume
   perspt resume

See Also
--------

- :doc:`../howto/agent-options` - Full CLI reference
- :doc:`../concepts/srbn-architecture` - SRBN details
