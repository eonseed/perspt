Architecture
============

Perspt is built as a modern, modular Rust application using a **6-crate workspace architecture**. 
This design enables clean separation of concerns, independent testing, and easy extensibility.

Workspace Overview
------------------

.. graphviz::
   :align: center
   :caption: Perspt Workspace Structure

   digraph workspace {
       rankdir=TB;
       node [shape=folder, style=filled, fontname="Helvetica"];
       
       subgraph cluster_workspace {
           label="perspt/crates/";
           style=dashed;
           color=gray;
           
           cli [label="perspt-cli\n(CLI Entry)", fillcolor="#4ECDC4"];
           core [label="perspt-core\n(LLM, Config)", fillcolor="#45B7D1"];
           tui [label="perspt-tui\n(Terminal UI)", fillcolor="#96CEB4"];
           agent [label="perspt-agent\n(SRBN Engine)", fillcolor="#FFEAA7"];
           policy [label="perspt-policy\n(Security)", fillcolor="#DDA0DD"];
           sandbox [label="perspt-sandbox\n(Isolation)", fillcolor="#F8B739"];
       }
   }

Crate Dependency Graph
----------------------

.. graphviz::
   :align: center
   :caption: Crate Dependencies

   digraph dependencies {
       rankdir=TB;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=11];
       edge [color="#666666"];
       
       cli [label="perspt-cli\n━━━━━━━━━━━\n8 Subcommands", fillcolor="#4ECDC4"];
       
       subgraph cluster_middle {
           rank=same;
           style=invis;
           tui [label="perspt-tui\n━━━━━━━━━━━\nAgent UI\nDashboard\nDiff Viewer", fillcolor="#96CEB4"];
           agent [label="perspt-agent\n━━━━━━━━━━━\nOrchestrator\nLSP Client\nTools", fillcolor="#FFEAA7"];
           core [label="perspt-core\n━━━━━━━━━━━\nGenAIProvider\nConfig\nMemory", fillcolor="#45B7D1"];
       }
       
       subgraph cluster_bottom {
           rank=same;
           style=invis;
           policy [label="perspt-policy\n━━━━━━━━━━━\nPolicyEngine\nSanitizer", fillcolor="#DDA0DD"];
           sandbox [label="perspt-sandbox\n━━━━━━━━━━━\nSandboxedCommand", fillcolor="#F8B739"];
       }
       
       cli -> tui;
       cli -> agent;
       cli -> core;
       agent -> policy;
       agent -> sandbox;
       agent -> core;
   }

SRBN Control Flow
-----------------

.. graphviz::
   :align: center
   :caption: Stabilized Recursive Barrier Network

   digraph srbn {
       rankdir=LR;
       node [shape=box, style="rounded,filled", fontname="Helvetica"];
       edge [fontname="Helvetica", fontsize=10];
       
       start [shape=circle, label="", fillcolor="#333333", width=0.3];
       
       sheaf [label="1. Sheafification\n━━━━━━━━━━━━━━\nTask → TaskPlan\n(Architect)", fillcolor="#E8F5E9"];
       spec [label="2. Speculation\n━━━━━━━━━━━━━━\nGenerate Code\n(Actuator)", fillcolor="#E3F2FD"];
       verify [label="3. Verification\n━━━━━━━━━━━━━━\nCompute V(x)\n(LSP + Tests)", fillcolor="#FFF3E0"];
       
       converge [shape=diamond, label="V(x) > ε?", fillcolor="#FFECB3"];
       
       commit [label="5. Commit\n━━━━━━━━━━━━━━\nMerkle Ledger\n(Record)", fillcolor="#F3E5F5"];
       
       end [shape=doublecircle, label="", fillcolor="#333333", width=0.3];
       
       start -> sheaf;
       sheaf -> spec;
       spec -> verify;
       verify -> converge;
       converge -> spec [label="Yes\n(retry)", style=dashed, color="#E53935"];
       converge -> commit [label="No\n(stable)"];
       commit -> end;
   }

Crate Details
-------------

perspt-cli
~~~~~~~~~~

The command-line interface providing 8 subcommands:

.. list-table:: CLI Subcommands
   :header-rows: 1
   :widths: 15 30 55
   :class: longtable

   * - Command
     - Purpose
     - Key Options
   * - ``chat``
     - Interactive TUI
     - ``--model <MODEL>``
   * - ``agent``
     - SRBN autonomous coding
     - | ``--architect-model``, ``--actuator-model``
       | ``--energy-weights``, ``--mode``
   * - ``init``
     - Project setup
     - ``--memory``, ``--rules``
   * - ``config``
     - Configuration
     - ``--show``, ``--set``, ``--edit``
   * - ``ledger``
     - Merkle ledger
     - ``--recent``, ``--rollback``, ``--stats``
   * - ``status``
     - Agent status
     - *(none)*
   * - ``abort``
     - Cancel session
     - ``--force``
   * - ``resume``
     - Resume session
     - ``[SESSION_ID]``

**Source**: :file:`crates/perspt-cli/src/`

perspt-core
~~~~~~~~~~~

Thread-safe LLM provider and configuration:

.. code-block:: rust
   :caption: GenAIProvider - Thread-safe LLM abstraction

   /// Thread-safe LLM provider using Arc<RwLock>.
   /// Can be safely cloned and shared across async tasks.
   #[derive(Clone)]
   pub struct GenAIProvider {
       client: Arc<Client>,
       shared: Arc<RwLock<SharedState>>,
   }

   impl GenAIProvider {
       pub fn new() -> Result<Self>
       pub fn new_with_config(provider: Option<&str>, api_key: Option<&str>) -> Result<Self>
       pub async fn generate_response_stream_to_channel(...) -> Result<()>
       pub async fn get_total_tokens_used(&self) -> usize
   }

**Modules**:

:config.rs: Simple Config struct (provider, model, api_key)
:llm_provider.rs: GenAIProvider with streaming support
:memory.rs: Conversation memory management

**Source**: :file:`crates/perspt-core/src/`

perspt-agent
~~~~~~~~~~~~

The Stabilized Recursive Barrier Network implementation.

.. admonition:: Energy Computation
   :class: tip

   .. math::

      V(x) = \alpha \cdot V_{syn} + \beta \cdot V_{str} + \gamma \cdot V_{log}

   **Default weights**: α=1.0, β=0.5, γ=2.0

**Key Modules**:

.. list-table::
   :widths: 25 10 65
   :header-rows: 1

   * - Module
     - Size
     - Description
   * - :file:`orchestrator.rs`
     - 34KB
     - SRBN control loop, model tiers, retry policy
   * - :file:`lsp.rs`
     - 28KB
     - LSP client for Python (``ty`` server)
   * - :file:`tools.rs`
     - 12KB
     - Agent tools (search, read, write, shell)
   * - :file:`types.rs`
     - 24KB
     - TaskPlan, Node, Energy, ToolCall types
   * - :file:`ledger.rs`
     - 6KB
     - Merkle ledger for change tracking
   * - :file:`test_runner.rs`
     - 15KB
     - pytest integration, V_log calculation
   * - :file:`context_retriever.rs`
     - 10KB
     - Code context extraction

**Source**: :file:`crates/perspt-agent/src/`

perspt-tui
~~~~~~~~~~

Ratatui-based terminal interface components:

:agent_app.rs: Main agent mode TUI application
:dashboard.rs: Status dashboard with metrics
:diff_viewer.rs: Side-by-side file diff display
:review_modal.rs: Change approval/rejection UI
:task_tree.rs: Hierarchical task visualization

**Source**: :file:`crates/perspt-tui/src/`

perspt-policy
~~~~~~~~~~~~~

Starlark-based policy engine for command approval:

.. code-block:: rust
   :caption: Security policy engine

   pub struct PolicyEngine {
       // Evaluates Starlark rules for command safety
   }

   pub struct Sanitizer {
       // Cleans and validates shell commands
       // Prevents path traversal, injection attacks
   }

**Source**: :file:`crates/perspt-policy/src/`

perspt-sandbox
~~~~~~~~~~~~~~

Safe command execution with process isolation.

**Source**: :file:`crates/perspt-sandbox/src/`

Design Principles
-----------------

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: 🧩 Modularity
      
      Each crate has a single responsibility:
      
      - **perspt-cli** knows CLI, not LLM internals
      - **perspt-core** provides LLM abstraction, not UI
      - **perspt-agent** implements SRBN, delegates UI

   .. grid-item-card:: 🔒 Thread Safety
      
      ``GenAIProvider`` uses ``Arc<RwLock<SharedState>>`` for:
      
      - Safe cloning across async tasks
      - Shared token counting and rate limiting
      - Concurrent access from orchestrator and UI

   .. grid-item-card:: ⚠️ Error Handling
      
      All crates use ``anyhow::Result`` for:
      
      - Contextual error messages
      - Error propagation with backtrace
      - User-friendly error display

   .. grid-item-card:: ⚡ Async Architecture
      
      Built on Tokio runtime with:
      
      - Streaming LLM responses via channels
      - Non-blocking UI updates
      - Concurrent tool execution

Configuration Sources
---------------------

.. list-table:: Configuration Priority (highest first)
   :header-rows: 1
   :widths: 10 30 60

   * - Priority
     - Source
     - Example
   * - 1
     - CLI Arguments
     - ``perspt agent --model gpt-5.2``
   * - 2
     - Environment Variables
     - ``OPENAI_API_KEY=sk-...``
   * - 3
     - Config File
     - ``~/.perspt/config.toml``
   * - 4
     - Built-in Defaults
     - provider: openai, model: gpt-4

Supported Providers
~~~~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 20 35 45

   * - Provider
     - Environment Variable
     - Models
   * - OpenAI
     - ``OPENAI_API_KEY``
     - GPT-5.2, o3-mini, o1-preview
   * - Anthropic
     - ``ANTHROPIC_API_KEY``
     - Claude Opus 4.5
   * - Google
     - ``GEMINI_API_KEY``
     - Gemini 3 Flash/Pro
   * - Groq
     - ``GROQ_API_KEY``
     - Llama 3.x
   * - Ollama
     - *(none)*
     - Local models

Extension Points
----------------

Adding a New Command
~~~~~~~~~~~~~~~~~~~~

1. Create :file:`crates/perspt-cli/src/commands/mycommand.rs`
2. Add variant to ``Commands`` enum in :file:`main.rs`
3. Add match arm in ``main()``

Adding a New Tool
~~~~~~~~~~~~~~~~~

1. Add tool definition to :file:`crates/perspt-agent/src/tools.rs`
2. Register in ``AgentTools::available_tools()``
3. Implement execution in ``execute_tool()``

Adding a Provider
~~~~~~~~~~~~~~~~~

The ``genai`` crate handles providers. To customize:

1. Set appropriate environment variable
2. Use provider-specific model names

.. seealso::

   - :doc:`../api/index` - Per-crate API reference
   - :doc:`contributing` - How to contribute
   - :doc:`testing` - Testing guide
