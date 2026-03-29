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

Widgets
-------

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Widget
     - Description
   * - ``Dashboard``
     - Main agent dashboard layout with panels
   * - ``TaskTree``
     - DAG visualization showing node states and energy
   * - ``ReviewModal``
     - Grouped diff viewer with approve/reject/correct controls
   * - ``DiffViewer``
     - Unified diff display with syntax highlighting
   * - ``LogsViewer``
     - LLM call log browser with filtering
   * - ``Theme``
     - Color scheme and styling

Entry Points
------------

.. code-block:: rust

   pub fn run_chat_tui(...) -> Result<()>;
   pub fn run_agent_tui_with_orchestrator(...) -> Result<()>;
   pub fn run_logs_viewer(...) -> Result<()>;
   pub fn init_terminal() -> Result<TuiTerminal>;
   pub fn restore_terminal(terminal: TuiTerminal) -> Result<()>;

Channels
--------

.. code-block:: rust

   pub fn create_app_event_channel() -> (AppEventSender, AppEventReceiver);
   pub fn create_telemetry_channel() -> (TelemetrySender, TelemetryReceiver);
