.. _howto-dashboard-setup:

Dashboard Setup
===============

Configure and launch the Perspt web dashboard for monitoring agent
sessions.

Locate the Config File
----------------------

The Perspt configuration file lives in your platform's config directory:

- **Linux**: ``~/.config/perspt/config.toml``
- **macOS**: ``~/Library/Application Support/perspt/config.toml``

If migrating from the legacy ``~/.perspt/`` layout, Perspt will warn and
fall back to the old location automatically.

Configure a Dashboard Password
-------------------------------

Add to ``config.toml``:

.. code-block:: toml

   [dashboard]
   password = "your-secret"

Without this section, the dashboard runs in open-access mode (suitable
for localhost-only use).

Change the Default Port
-----------------------

The default port is ``3000``. Override via CLI flag:

.. code-block:: bash

   perspt dashboard --port 8080

Build from Source
-----------------

.. code-block:: bash

   cargo build -p perspt-dashboard --release

The dashboard binary is built as part of the ``perspt`` CLI. Running
``cargo build --release`` from the workspace root includes it.

Launch Locally
--------------

.. code-block:: bash

   perspt dashboard

The server binds to ``127.0.0.1:3000`` by default. Open
``http://localhost:3000`` in your browser.

Point to a Specific Database
----------------------------

By default, the dashboard reads from the platform data directory
(``~/.local/share/perspt/perspt.db`` on Linux). To use a different
database file:

.. code-block:: bash

   perspt dashboard --db-path /path/to/perspt.db

Embed in Agent Mode
-------------------

Instead of running the dashboard as a separate process, you can start it
alongside the agent using the ``--dashboard`` flag:

.. code-block:: bash

   perspt agent --dashboard "Create a REST API"

This spawns the dashboard as a background task within the agent process.
It opens a separate read-only DuckDB connection to the same database file
the agent writes to — DuckDB supports one writer plus concurrent readers.

To use a custom port:

.. code-block:: bash

   perspt agent --dashboard --dashboard-port 8080 "Add tests"

The embedded dashboard provides the same interface as the standalone
``perspt dashboard`` command: DAG topology, energy convergence,
LLM telemetry, and correction-attempt provenance. It stops
automatically when the agent process exits.

.. note::

   The embedded dashboard is especially useful for headless CI runs
   (``--yes --dashboard``) where you want a browser view of progress
   without a separate terminal.
