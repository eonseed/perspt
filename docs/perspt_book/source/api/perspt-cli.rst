.. _api-perspt-cli:

``perspt-cli``
==============

Clap-based CLI entry point with sixteen subcommands, plus one feature-gated.

Subcommands
-----------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Command
     - Description
   * - ``chat``
     - TUI chat (default)
   * - ``simple-chat``
     - Plain-text streaming chat
   * - ``agent``
     - PSP-9 autonomous coding agent (22 options)
   * - ``init``
     - Initialize project memory and policy rules
   * - ``config``
     - View/set/edit configuration (``--show``/``--set``/``--edit``)
   * - ``ledger``
     - Query and manage the Merkle ledger
       (``--recent``/``--rollback <session-prefix>``/``--stats``)
   * - ``status``
     - Show current agent status
   * - ``audit``
     - Delayed audit labels and conformal activation (PSP-9)
   * - ``providers``
     - Print the provider capability matrix; ``--probe`` runs live probes
   * - ``replay``
     - Deterministic, credential-free audit replay of a session (PSP-9)
   * - ``abort``
     - Abort a PSP-9 session by revoking its authority epoch
   * - ``resume``
     - Resume a paused or crashed session
   * - ``dashboard``
     - Launch the web monitoring dashboard
   * - ``db``
     - Inspect and repair the local DuckDB store (``repair``)
   * - ``prompts``
     - Inspect and maintain the compiled prompt section libraries
       (``list``/``render``/``lint``/``manifest``/``explain-session``)
   * - ``context``
     - Explain a session's recorded resident-context events
       (``explain-turn``)
   * - ``benchmark``
     - Optional model-backed evaluation tooling
       (``validate``/``run``/``aggregate``); feature-gated behind the
       ``benchmark`` feature and run manually, never in CI

See :doc:`../reference/cli-reference` for the complete flag reference.
