.. _agent-options:

Agent Mode Options
==================

Complete reference for ``perspt agent`` command.

Basic Usage
-----------

.. code-block:: bash

   perspt agent [OPTIONS] <TASK>

Options
-------

.. list-table::
   :widths: 30 15 55
   :header-rows: 1

   * - Option
     - Default
     - Description
   * - ``-w, --workspace <DIR>``
     - ``./``
     - Working directory for generated files
   * - ``-y, --yes``
     - false
     - Auto-approve all actions
   * - ``-k, --complexity <K>``
     - 5
     - Max tasks before requiring approval
   * - ``--architect-model <M>``
     - (provider default)
     - Model for task decomposition
   * - ``--actuator-model <M>``
     - (provider default)
     - Model for code generation
   * - ``--max-tokens <N>``
     - 100000
     - Token budget limit
   * - ``--max-cost <USD>``
     - (none)
     - Maximum cost in dollars

Examples
--------

**Basic code generation**:

.. code-block:: bash

   perspt agent "Create a Python calculator"

**With workspace and auto-approve**:

.. code-block:: bash

   perspt agent -w ./myproject -y "Add unit tests"

**Cost-controlled**:

.. code-block:: bash

   perspt agent --max-cost 0.50 "Write a simple script"

**Different models for planning vs coding**:

.. code-block:: bash

   perspt agent \
     --architect-model gpt-4o \
     --actuator-model gpt-4o-mini \
     "Implement binary search tree"

Token Budget
------------

The token budget tracks total usage across all LLM calls:

- Input tokens (prompts)
- Output tokens (responses)
- Estimated cost

When exhausted, agent stops and reports status.

Retry Behavior
--------------

Per PSP-000004:

- **Compilation errors**: 3 retries → escalate
- **Tool failures**: 5 retries → escalate
- **Review rejections**: 3 retries → escalate
