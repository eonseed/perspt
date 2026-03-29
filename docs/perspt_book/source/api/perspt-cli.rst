.. _api-perspt-cli:

``perspt-cli``
==============

Clap-based CLI entry point with 10 subcommands.

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
     - SRBN autonomous coding agent (40+ options)
   * - ``init``
     - Initialize project configuration
   * - ``config``
     - View/edit/reset configuration
   * - ``ledger``
     - Query Merkle ledger
   * - ``status``
     - Show current session state
   * - ``abort``
     - Abort active session
   * - ``resume``
     - Resume interrupted session
   * - ``logs``
     - View LLM call logs

See :doc:`../reference/cli-reference` for the complete flag reference.
