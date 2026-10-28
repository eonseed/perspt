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
   * - ``-v, --verbose``
     - Enable verbose logging
   * - ``-c, --config <PATH>``
     - Path to the TOML configuration file
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

   perspt chat [--model MODEL]


``simple-chat``
~~~~~~~~~~~~~~~

Launch the plain-text CLI chat.

.. code-block:: bash

   perspt simple-chat [--model MODEL] [--log-file FILE]


``dashboard``
~~~~~~~~~~~~~

Launch the real-time web monitoring dashboard.

.. code-block:: bash

   perspt dashboard [--port PORT] [--db-path PATH]

- ``--port`` - HTTP port (default ``3000``)
- ``--db-path`` - Path to DuckDB database file (default: platform data directory)

The server always binds to ``127.0.0.1``.

See :doc:`../howto/dashboard-setup` for configuration details.


``agent``
~~~~~~~~~

Run the SRBN autonomous coding agent.

.. code-block:: bash

   perspt agent [OPTIONS] <TASK>

- ``--dashboard`` - Start the web monitoring dashboard alongside the agent
- ``--dashboard-port <PORT>`` - Port for the embedded dashboard (default ``3000``)
- ``--allow-unisolated`` - Explicit reduced-isolation process execution;
  required for native Windows coding until a sandbox backend exists

See :doc:`../howto/agent-options` for full agent options.


``init``
~~~~~~~~

Initialize project memory and policy rules.

.. code-block:: bash

   perspt init [--memory] [--rules]

- ``--memory`` - Create the ``PERSPT.md`` project memory file
- ``--rules`` - Create default Starlark policy rules


``config``
~~~~~~~~~~

View or edit Perspt configuration.

.. code-block:: bash

   perspt config [--show] [--set KEY=VALUE] [--edit]

- ``--show`` - Print the effective config (``api_key`` masked)
- ``--set KEY=VALUE`` - Set a value with a structured TOML write
- ``--edit`` - Open the config file in ``$EDITOR``


``ledger``
~~~~~~~~~~

Query the Merkle ledger.

.. code-block:: bash

   perspt ledger [--recent] [--stats] [--rollback SESSION_PREFIX]

- ``--recent`` - Show recent commits
- ``--stats`` - Show ledger statistics
- ``--rollback SESSION_PREFIX`` - Undo the session's newest completed
  promotion and label it unsafe


``status``
~~~~~~~~~~

Show current session status.

.. code-block:: bash

   perspt status

Displays: ledger event count, ledger head, measurement count, last energy,
last gate decision, denial count, search forest/branch/no-good counters,
validator verdicts, the Φ(W) conditional-capacity diagnostic,
validator-independence statistics, pending external effects, and a
``perspt replay`` hint.


``abort``
~~~~~~~~~

Abort a PSP-9 session by revoking its authority epoch.

.. code-block:: bash

   perspt abort [--force] [SESSION_ID]

- ``-f, --force`` - Force abort without confirmation
- ``SESSION_ID`` - Session to abort (defaults to the newest running PSP-9
  session)


``resume``
~~~~~~~~~~

Resume an interrupted session.

.. code-block:: bash

   perspt resume [SESSION_ID] [--last] [--db-path PATH]

- ``SESSION_ID`` - Session ID to resume
- ``--last`` - Resume the most recent session
- ``--db-path`` - Database file to inspect (defaults to the standard store)

Displays the session id, task, working directory, and status before
resuming, and verifies the ledger chain is valid.


``audit``
~~~~~~~~~

Delayed audit labels and conformal activation (PSP-9).

.. code-block:: bash

   perspt audit [SAMPLE] [--safe] [--unsafe]

- ``SAMPLE`` - Sample id (or unique prefix) to label; omit to list pending
  samples
- ``--safe`` - Label the sample as safe
- ``--unsafe`` - Label the sample as unsafe


``providers``
~~~~~~~~~~~~~

Print the provider capability matrix (PSP-9).

.. code-block:: bash

   perspt providers [--probe]

- ``--probe`` - Run live behavioral probes against every configured model
  route


``replay``
~~~~~~~~~~

Deterministic, credential-free audit replay of a session (PSP-9).

.. code-block:: bash

   perspt replay <SESSION_ID> [--db-path PATH]

- ``SESSION_ID`` - The session id to replay
- ``--db-path`` - Database file to inspect (defaults to the standard store)


``db``
~~~~~~

Inspect and repair the local DuckDB store.

.. code-block:: bash

   perspt db repair --db-path PATH [--discard-wal]

- ``repair`` - Quarantine a poisoned WAL after making durable backups
- ``--db-path`` - Database file to repair
- ``--discard-wal`` - Explicitly authorize WAL quarantine; the WAL is never
  deleted


``prompts``
~~~~~~~~~~~

Inspect and maintain the compiled prompt section libraries (PSP-10).

.. code-block:: bash

   perspt prompts list
   perspt prompts render <STAGE>
   perspt prompts lint [--bundle DIR]
   perspt prompts manifest <DIR>
   perspt prompts explain-session --db-path PATH <SESSION_ID>

- ``list`` - List every compiled section: id, version, stage, role, hash
- ``render <STAGE>`` - Compose one stage with fixture variables and print it
- ``lint`` - Run the codegen validation list over an external bundle
  directory
- ``manifest <DIR>`` - Regenerate a prompt library's committed
  ``manifest.toml`` (explicit)
- ``explain-session`` - Show the programs a session actually compiled, with
  digests


``context``
~~~~~~~~~~~

Explain a session's recorded resident-context events (PSP-10).

.. code-block:: bash

   perspt context explain-turn --db-path PATH <SESSION_ID>

- ``explain-turn`` - Show a session's recorded context events (compactions,
  refusals)


``benchmark`` (optional)
~~~~~~~~~~~~~~~~~~~~~~~~

Build ``perspt-cli`` with the Cargo feature ``benchmark`` to expose the
separate evaluation runner. It is absent from the default CLI build.

.. code-block:: bash

   perspt benchmark validate
   perspt --config config.local.toml benchmark run --suite smoke --output report.json
   perspt benchmark aggregate report-a.json report-b.json

``validate`` is credential-free. ``run`` is explicit and credentialed; its
``smoke``, ``adaptive``, and ``full`` suites use production role resolution
from the selected Perspt configuration and record the configured topology.
``run --tasks <N>`` overrides the suite's task count. Coding verification
stays deterministic, so a configured verifier route is provenance rather
than a model call. Model names and family labels are not benchmark
arguments. The benchmark is manual-only and feature-gated — the
``perspt-benchmark`` crate is a separate optional crate — and a benchmark
run is never part of normal validation or CI.
