perspt-tui API
==============

Terminal UI components for Perspt built on the Ratatui framework.

Overview
--------

``perspt-tui`` provides the visual interface for chat, agent, and logs modes:

- **ChatApp** — Interactive chat TUI
- **AgentApp** — Agent mode dashboard
- **LogsViewer** — LLM logs analysis TUI
- **Dashboard**, **DiffViewer**, **ReviewModal**, **TaskTree** — UI components

.. graphviz::
   :align: center
   :caption: TUI Component Architecture

   digraph tui {
       rankdir=TB;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=11];
       
       app [label="AgentApp\n━━━━━━━━━━\nMain Application", fillcolor="#4ECDC4"];
       
       subgraph cluster_components {
           label="UI Components";
           style=dashed;
           color=gray;
           
           dashboard [label="Dashboard\n━━━━━━━━━━\nStatus & Metrics", fillcolor="#96CEB4"];
           tree [label="TaskTree\n━━━━━━━━━━\nTask Hierarchy", fillcolor="#FFEAA7"];
           diff [label="DiffViewer\n━━━━━━━━━━\nFile Changes", fillcolor="#DDA0DD"];
           review [label="ReviewModal\n━━━━━━━━━━\nApproval UI", fillcolor="#F8B739"];
       }
       
       app -> dashboard;
       app -> tree;
       app -> diff;
       app -> review;
   }

Modules
-------

AgentApp
~~~~~~~~

The main TUI application for agent mode.

.. code-block:: rust
   :caption: AgentApp structure

   pub struct AgentApp {
       // Current view state
       view: View,
       // Task tree widget
       task_tree: TaskTree,
       // Status dashboard
       dashboard: Dashboard,
       // Active diff viewer (if showing changes)
       diff_viewer: Option<DiffViewer>,
       // Review modal (if awaiting approval)
       review_modal: Option<ReviewModal>,
   }

   pub enum View {
       Dashboard,
       TaskTree,
       DiffView,
       Review,
   }

   impl AgentApp {
       pub fn new() -> Self
       pub fn run(&mut self, terminal: &mut Terminal<impl Backend>) -> Result<()>
       pub fn handle_event(&mut self, event: Event) -> Result<Option<Action>>
   }

.. function:: run_agent_tui(orchestrator: SRBNOrchestrator) -> Result<()>

   Entry point for the agent TUI. Initializes the terminal and runs the event loop.

Dashboard
~~~~~~~~~

Status dashboard displaying metrics and progress.

.. list-table:: Dashboard Widgets
   :header-rows: 1
   :widths: 25 75

   * - Widget
     - Information Displayed
   * - Session Info
     - Session ID, start time, elapsed duration
   * - Token Usage
     - Input/output tokens, cost estimate
   * - Energy Gauge
     - Current V(x) with α, β, γ components
   * - Task Progress
     - Completed/total nodes, current node

.. code-block:: rust
   :caption: Dashboard structure

   pub struct Dashboard {
       session_id: String,
       start_time: Instant,
       tokens_used: usize,
       current_energy: Energy,
       task_progress: (usize, usize),
   }

   impl Dashboard {
       pub fn render(&self, frame: &mut Frame, area: Rect)
       pub fn update_energy(&mut self, energy: Energy)
       pub fn update_tokens(&mut self, tokens: usize)
   }

TaskTree
~~~~~~~~

Hierarchical task visualization.

.. code-block:: rust
   :caption: TaskTree widget

   pub struct TaskTree {
       nodes: Vec<TreeNode>,
       selected: usize,
       expanded: HashSet<usize>,
   }

   pub struct TreeNode {
       pub id: usize,
       pub label: String,
       pub status: NodeStatus,
       pub children: Vec<usize>,
   }

   impl TaskTree {
       pub fn from_task_plan(plan: &TaskPlan) -> Self
       pub fn render(&self, frame: &mut Frame, area: Rect)
       pub fn select_next(&mut self)
       pub fn select_prev(&mut self)
       pub fn toggle_expand(&mut self)
   }

DiffViewer
~~~~~~~~~~

Side-by-side file diff display.

.. code-block:: rust
   :caption: DiffViewer widget

   pub struct DiffViewer {
       file_path: PathBuf,
       old_content: String,
       new_content: String,
       scroll_offset: usize,
   }

   impl DiffViewer {
       pub fn new(path: PathBuf, old: String, new: String) -> Self
       pub fn render(&self, frame: &mut Frame, area: Rect)
       pub fn scroll_up(&mut self)
       pub fn scroll_down(&mut self)
   }

ReviewModal
~~~~~~~~~~~

Change approval/rejection modal.

.. code-block:: rust
   :caption: ReviewModal widget

   pub struct ReviewModal {
       changes: Vec<Change>,
       selected: usize,
   }

   pub enum ReviewDecision {
       Approve,
       Reject,
       Edit,
   }

   impl ReviewModal {
       pub fn new(changes: Vec<Change>) -> Self
       pub fn render(&self, frame: &mut Frame, area: Rect)
       pub fn get_decision(&self) -> Option<ReviewDecision>
   }

LogsViewer
~~~~~~~~~~

Interactive TUI for analyzing LLM request/response logs.

.. code-block:: rust
   :caption: LogsViewer structure

   pub struct LogsViewer {
       store: SessionStore,
       sessions: Vec<SessionRecord>,
       requests: Vec<LlmRequestRecord>,
       active_panel: ActivePanel,
       detail_tab: DetailTab,
   }

   pub enum ActivePanel {
       Sessions,
       Requests,
       Detail,
   }

   pub enum DetailTab {
       Prompt,
       Response,
       Stats,
   }

   impl LogsViewer {
       pub fn new(store: SessionStore) -> Self
       pub fn handle_key(&mut self, key: KeyCode)
       pub fn render(&mut self, frame: &mut Frame)
   }

.. function:: run_logs_viewer(store: SessionStore) -> Result<()>

   Entry point for the logs viewer TUI.

Telemetry
~~~~~~~~~

Real-time telemetry events from the orchestrator.

.. code-block:: rust

   pub enum TelemetryEvent {
       EnergyUpdate(EnergyComponents),
       TokensUsed { input: usize, output: usize },
       StepComplete { node_id: String },
   }

   pub struct EnergyComponents {
       pub v_syn: f32,
       pub v_str: f32,
       pub v_log: f32,
       pub total: f32,
   }

   pub fn create_telemetry_channel() -> (TelemetrySender, TelemetryReceiver)

Key Bindings
------------

.. list-table::
   :header-rows: 1
   :widths: 20 40 40

   * - Key
     - Dashboard/Tree View
     - Diff/Review View
   * - ``q`` / ``Esc``
     - Exit application
     - Close modal
   * - ``↑`` / ``k``
     - Select previous
     - Scroll up
   * - ``↓`` / ``j``
     - Select next
     - Scroll down
   * - ``Enter``
     - Expand/view details
     - Confirm action
   * - ``Tab``
     - Switch view
     - Switch pane
   * - ``y``
     - —
     - Approve change
   * - ``n``
     - —
     - Reject change

Source Code
-----------

:file:`crates/perspt-tui/src/agent_app.rs`: Main application (17KB)
:file:`crates/perspt-tui/src/chat_app.rs`: Chat TUI (30KB)
:file:`crates/perspt-tui/src/logs_viewer.rs`: Logs viewer (35KB)
:file:`crates/perspt-tui/src/dashboard.rs`: Status dashboard (8KB)
:file:`crates/perspt-tui/src/diff_viewer.rs`: Diff display (20KB)
:file:`crates/perspt-tui/src/review_modal.rs`: Review UI (15KB)
:file:`crates/perspt-tui/src/task_tree.rs`: Task hierarchy (15KB)
:file:`crates/perspt-tui/src/telemetry.rs`: Telemetry events (4KB)
