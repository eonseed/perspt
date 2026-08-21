.. _api-perspt-tui:

``perspt-tui``
==============

Ratatui-based terminal user interface with two application modes.

Applications
------------

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Type
     - Description
   * - ``ChatApp``
     - Interactive chat with markdown rendering and response streaming
   * - ``AgentApp``
     - Agent dashboard with DAG tree, energy display, and review modal

Widgets and Rendering Modules
-----------------------------

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Module
     - Description
   * - ``dashboard``
     - ``Dashboard`` - main agent dashboard layout with panels
   * - ``task_tree``
     - DAG visualization showing node states and energy
   * - ``review_modal``
     - ``ReviewModal`` - grouped diff viewer with approve/reject/correct controls
   * - ``diff_viewer``
     - ``DiffViewer`` - unified diff display with syntax highlighting
   * - ``simple_input``
     - Minimal input widget with full control over key handling
   * - ``markdown``
     - Markdown, table, and LaTeX-math rendering for the chat TUI
   * - ``latex``
     - LaTeX to Unicode transpilation for terminal math rendering
   * - ``chat_view``
     - Message-area and input-box rendering for the chat TUI
   * - ``chat_commands``
     - Chat-local slash commands and conversation export
   * - ``theme``
     - ``Theme`` - color scheme and styling

Entry Points
------------

.. code-block:: rust

   pub fn run_chat_tui(...) -> Result<()>;
   pub fn run_agent_tui_with_runtime(...) -> Result<()>;

Terminal setup and teardown live in ``tui_runner``:

.. code-block:: rust

   pub fn init_terminal(config: &TuiRunnerConfig) -> io::Result<TuiTerminal>;
   pub fn restore_terminal(config: &TuiRunnerConfig) -> io::Result<()>;

Channels
--------

.. code-block:: rust

   pub fn create_app_event_channel() -> (AppEventSender, AppEventReceiver);
   pub fn create_telemetry_channel() -> (TelemetrySender, TelemetryReceiver);
