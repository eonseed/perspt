perspt-agent API
================

The SRBN (Stabilized Recursive Barrier Network) engine for autonomous coding.

Overview
--------

``perspt-agent`` implements the core autonomous coding capabilities:

- **SRBNOrchestrator** - Main control loop for task execution
- **LspClient** - Language Server Protocol integration
- **AgentTools** - File and shell operations
- **PythonTestRunner** - pytest integration with V_log calculation
- **MerkleLedger** - Change tracking with integrity verification

SRBN Control Loop
-----------------

The orchestrator follows the SRBN algorithm:

.. graphviz::
   :align: center
   :caption: SRBN Control Flow

   digraph srbn {
       rankdir=LR;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=11];
       edge [fontname="Helvetica", fontsize=10];
       
       start [shape=point, width=0.2];
       
       sheaf [label="Sheafification\n━━━━━━━━━━━━\nTask → TaskPlan", fillcolor="#E8F5E9"];
       spec [label="Speculation\n━━━━━━━━━━━━\nGenerate Code", fillcolor="#E3F2FD"];
       verify [label="Verification\n━━━━━━━━━━━━\nCompute V(x)", fillcolor="#FFF3E0"];
       
       converge [shape=diamond, label="V(x) > ε?", fillcolor="#FFECB3"];
       
       commit [label="Commit\n━━━━━━━━━━━━\nMerkle Ledger", fillcolor="#F3E5F5"];
       
       end [shape=doublecircle, width=0.2, label=""];
       
       start -> sheaf;
       sheaf -> spec;
       spec -> verify;
       verify -> converge;
       converge -> spec [label="Yes (retry)", style=dashed, color="#E53935"];
       converge -> commit [label="No (stable)"];
       commit -> end;
   }

SRBNOrchestrator
----------------

The main orchestrator class:

.. code-block:: rust

   pub struct SRBNOrchestrator {
       provider: Arc<GenAIProvider>,
       workspace: PathBuf,
       lsp_client: Option<LspClient>,
       test_runner: Option<PythonTestRunner>,
       ledger: MerkleLedger,
       tools: AgentTools,
       
       // Model configuration
       architect_model: String,
       actuator_model: String,
       verifier_model: String,
       speculator_model: String,
       
       // Energy weights
       alpha: f32,  // V_syn weight (default: 1.0)
       beta: f32,   // V_str weight (default: 0.5)
       gamma: f32,  // V_log weight (default: 2.0)
       
       // Convergence threshold
       epsilon: f32,  // Default: 0.1
   }

Constructor
~~~~~~~~~~~

.. code-block:: rust

   impl SRBNOrchestrator {
       pub async fn new(
           provider: Arc<GenAIProvider>,
           workspace: PathBuf,
           options: OrchestratorOptions,
       ) -> Result<Self>
   }

   pub struct OrchestratorOptions {
       pub architect_model: Option<String>,
       pub actuator_model: Option<String>,
       pub verifier_model: Option<String>,
       pub speculator_model: Option<String>,
       pub alpha: f32,
       pub beta: f32,
       pub gamma: f32,
       pub epsilon: f32,
       pub max_retries_compile: usize,  // Default: 3
       pub max_retries_tool: usize,     // Default: 5
   }

Main Execution
~~~~~~~~~~~~~~

.. code-block:: rust

   impl SRBNOrchestrator {
       /// Execute a task through the SRBN loop
       pub async fn execute(&mut self, task: &str) -> Result<ExecutionResult>
       
       /// Execute with approval callback for complexity > K
       pub async fn execute_with_approval<F>(
           &mut self,
           task: &str,
           complexity_k: usize,
           approval_fn: F,
       ) -> Result<ExecutionResult>
       where
           F: Fn(&TaskPlan) -> bool
   }

Energy Computation
------------------

Lyapunov Energy V(x):

.. math::

   V(x) = α \cdot V_{syn} + β \cdot V_{str} + γ \cdot V_{log}

Components:

- **V_syn** - Syntax energy from LSP diagnostics
- **V_str** - Structural energy from code analysis
- **V_log** - Logic energy from test failures

.. code-block:: rust

   pub struct Energy {
       pub v_syn: f32,
       pub v_str: f32,
       pub v_log: f32,
       pub total: f32,
   }

   impl Energy {
       pub fn compute(
           lsp_diagnostics: &[Diagnostic],
           test_results: &TestResults,
           alpha: f32,
           beta: f32,
           gamma: f32,
       ) -> Self
   }

Types
-----

TaskPlan
~~~~~~~~

.. code-block:: rust

   /// JSON-serializable task decomposition
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct TaskPlan {
       pub nodes: Vec<TaskNode>,
       pub dependencies: Vec<(usize, usize)>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct TaskNode {
       pub id: usize,
       pub description: String,
       pub node_type: NodeType,
       pub status: NodeStatus,
       pub files_affected: Vec<String>,
   }

   pub enum NodeType {
       Create,
       Modify,
       Delete,
       Test,
       Shell,
   }

   pub enum NodeStatus {
       Pending,
       InProgress,
       Completed,
       Failed(String),
   }

ToolCall
~~~~~~~~

.. code-block:: rust

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct ToolCall {
       pub name: String,
       pub arguments: serde_json::Value,
   }

   pub enum ToolResult {
       Success(String),
       Error(String),
   }

LspClient
---------

Language Server Protocol client for real-time diagnostics:

.. code-block:: rust

   pub struct LspClient {
       process: Child,
       reader: BufReader<ChildStdout>,
       writer: BufWriter<ChildStdin>,
   }

   impl LspClient {
       /// Start LSP server for Python using `ty`
       pub async fn new_python(workspace: &Path) -> Result<Self>
       
       /// Get diagnostics for a file
       pub async fn get_diagnostics(&mut self, file: &Path) -> Result<Vec<Diagnostic>>
       
       /// Notify file change
       pub async fn did_change(&mut self, file: &Path, content: &str) -> Result<()>
   }

   pub struct Diagnostic {
       pub severity: DiagnosticSeverity,
       pub message: String,
       pub range: Range,
   }

   pub enum DiagnosticSeverity {
       Error,
       Warning,
       Information,
       Hint,
   }

PythonTestRunner
----------------

pytest integration with V_log calculation:

.. code-block:: rust

   pub struct PythonTestRunner {
       workspace: PathBuf,
   }

   impl PythonTestRunner {
       pub fn new(workspace: PathBuf) -> Self
       
       /// Run pytest and compute V_log
       pub async fn run(&self) -> Result<TestResults>
   }

   pub struct TestResults {
       pub passed: usize,
       pub failed: usize,
       pub errors: usize,
       pub v_log: f32,
       pub failures: Vec<TestFailure>,
   }

   pub struct TestFailure {
       pub test_name: String,
       pub message: String,
       pub criticality: f32,  // Weight for V_log
   }

AgentTools
----------

Available tools for the agent:

.. list-table::
   :header-rows: 1

   * - Tool
     - Description
   * - ``read_file``
     - Read file contents
   * - ``write_file``
     - Write/create file
   * - ``search_files``
     - Search for patterns in files
   * - ``list_directory``
     - List directory contents
   * - ``execute_shell``
     - Run shell command (sandboxed)
   * - ``get_diagnostics``
     - Get LSP diagnostics
   * - ``run_tests``
     - Execute pytest

.. code-block:: rust

   pub struct AgentTools {
       workspace: PathBuf,
       policy_engine: Arc<PolicyEngine>,
       sandbox: SandboxedCommand,
   }

   impl AgentTools {
       pub fn available_tools() -> Vec<ToolDefinition>
       pub async fn execute(&self, call: &ToolCall) -> Result<ToolResult>
   }

MerkleLedger
------------

Git-style change tracking with Merkle tree:

.. code-block:: rust

   pub struct MerkleLedger {
       root: Option<Hash>,
       commits: Vec<Commit>,
   }

   impl MerkleLedger {
       pub fn new() -> Self
       
       /// Record a commit
       pub fn commit(&mut self, changes: Vec<Change>) -> Hash
       
       /// Get commit by hash
       pub fn get(&self, hash: &Hash) -> Option<&Commit>
       
       /// Rollback to previous commit
       pub fn rollback(&mut self, hash: &Hash) -> Result<()>
   }

Retry Policy
------------

PSP-4 compliant retry limits:

.. list-table::
   :header-rows: 1

   * - Error Type
     - Max Retries
     - Action on Exhaustion
   * - Compilation errors
     - 3
     - Escalate to user
   * - Tool failures
     - 5
     - Escalate to user
   * - Review rejections
     - 3
     - Escalate to user

Source Code
-----------

- ``crates/perspt-agent/src/orchestrator.rs`` (34KB)
- ``crates/perspt-agent/src/lsp.rs`` (28KB)
- ``crates/perspt-agent/src/tools.rs`` (12KB)
- ``crates/perspt-agent/src/types.rs`` (24KB)
- ``crates/perspt-agent/src/ledger.rs`` (6KB)
- ``crates/perspt-agent/src/test_runner.rs`` (15KB)
