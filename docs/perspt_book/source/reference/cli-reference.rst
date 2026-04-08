.. _reference-cli:

CLI Reference
=============

.. code-block:: text

   perspt [OPTIONS] [COMMAND]

Global Options
--------------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Flag
     - Description
   * - ``--config <PATH>``
     - Path to configuration file
   * - ``--api-key <KEY>``
     - API key for the LLM provider
   * - ``--provider-type <TYPE>``
     - Provider: ``openai``, ``anthropic``, ``gemini``, ``groq``, ``cohere``, ``xai``, ``deepseek``, ``ollama``
   * - ``--provider <NAME>``
     - Provider name (equivalent to ``--provider-type``)
   * - ``--model <MODEL>``
     - Model identifier
   * - ``--list-models``
     - List available models and exit
   * - ``-h, --help``
     - Print help
   * - ``-V, --version``
     - Print version


Commands
--------

``chat`` (default)
~~~~~~~~~~~~~~~~~~

Launch the TUI chat interface.

.. code-block:: bash

   perspt chat [--model MODEL] [--provider-type TYPE]


``simple-chat``
~~~~~~~~~~~~~~~

Launch the plain-text CLI chat.

.. code-block:: bash

   perspt simple-chat [--log-file FILE]


``dashboard``
~~~~~~~~~~~~~

Launch the real-time web monitoring dashboard.

.. code-block:: bash

   perspt dashboard [--port PORT] [--bind ADDR] [--db-path PATH]

- ``--port`` — HTTP port (default ``3000``)
- ``--bind`` — Bind address (default ``127.0.0.1``)
- ``--db-path`` — Path to DuckDB database file (default: platform data directory)

See :doc:`../howto/dashboard-setup` for configuration details.


``agent``
~~~~~~~~~

Run the SRBN autonomous coding agent.

.. code-block:: bash

   perspt agent [OPTIONS] <TASK>

- ``--dashboard`` — Start the web monitoring dashboard alongside the agent
- ``--dashboard-port <PORT>`` — Port for the embedded dashboard (default ``3000``)

See :doc:`../howto/agent-options` for full agent options.


``init``
~~~~~~~~

Initialize a new project with Perspt configuration.

.. code-block:: bash

   perspt init [--workdir DIR]


``config``
~~~~~~~~~~

View or edit Perspt configuration.

.. code-block:: bash

   perspt config [show|edit|reset]


``ledger``
~~~~~~~~~~

Query the Merkle ledger.

.. code-block:: bash

   perspt ledger [--recent] [--stats] [--node NODE_ID]


``status``
~~~~~~~~~~

Show current session status.

.. code-block:: bash

   perspt status

Displays: per-node lifecycle counts (queued, running, verifying, retrying,
completed, failed, escalated), latest energy breakdown, total retry count,
recent escalation reports, step timeline summary (per-step-type counts,
total step time), and correction attempt summaries (accepted/rejected counts
per node).


``abort``
~~~~~~~~~

Abort the current agent session.

.. code-block:: bash

   perspt abort


``resume``
~~~~~~~~~~

Resume an interrupted session.

.. code-block:: bash

   perspt resume [--last]

Displays trust context before resuming: escalation count, last energy state,
total retries. The ``BudgetEnvelope`` (step/cost/revision caps) is restored from
the database so limits continue from the interrupted session.


``logs``
~~~~~~~~

View LLM call logs and token metrics. Full prompt/response text is only
available when ``--log-llm`` was active during the session; basic token
usage, latency, and cost data are always recorded.

.. code-block:: bash

   perspt logs [--tui] [--last] [--stats]
