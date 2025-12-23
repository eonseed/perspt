perspt-cli API
==============

The command-line interface entry point with 8 subcommands.

Overview
--------

``perspt-cli`` provides the user-facing CLI with subcommand architecture:

- **chat** - Interactive TUI mode
- **agent** - SRBN autonomous coding
- **init** - Project initialization
- **config** - Configuration management
- **ledger** - Merkle ledger operations
- **status** - Agent status
- **abort** - Cancel session
- **resume** - Resume session

Global Options
--------------

These options apply to all subcommands:

.. code-block:: text

   perspt [OPTIONS] <COMMAND>

   Options:
     -v, --verbose        Enable verbose logging
     -c, --config <FILE>  Configuration file path
     -h, --help          Print help
     -V, --version       Print version

Chat Command
------------

Start an interactive TUI chat session.

.. code-block:: text

   perspt chat [OPTIONS]

   Options:
     -m, --model <MODEL>  Model to use (e.g., gpt-5.2)

Agent Command
-------------

Run the SRBN agent on a coding task.

.. code-block:: text

   perspt agent [OPTIONS] <TASK>

   Arguments:
     <TASK>  Task description or path to task file

   Model Selection:
     --model <MODEL>              Override all model tiers
     --architect-model <MODEL>    Model for planning (deep reasoning)
     --actuator-model <MODEL>     Model for code generation
     --verifier-model <MODEL>     Model for stability checking
     --speculator-model <MODEL>   Model for fast lookahead

   Execution Control:
     -w, --workdir <DIR>      Working directory (default: current)
     -y, --yes                Auto-approve all actions
     --auto-approve-safe      Auto-approve read-only operations
     -k, --complexity <K>     Max tasks before approval (default: 5)
     -m, --mode <MODE>        Execution mode: cautious | balanced | yolo

   SRBN Parameters:
     --energy-weights <α,β,γ>      Lyapunov weights (default: 1.0,0.5,2.0)
     --stability-threshold <ε>     Convergence threshold (default: 0.1)

   Limits:
     --max-cost <USD>         Maximum cost in dollars (0 = unlimited)
     --max-steps <N>          Maximum iterations (0 = unlimited)

Examples
~~~~~~~~

.. code-block:: bash

   # Basic task
   perspt agent "Create a Python calculator"

   # With workspace directory
   perspt agent -w /path/to/project "Add unit tests"

   # Auto-approve all
   perspt agent -y "Refactor the parser"

   # Custom models
   perspt agent --architect-model gpt-5.2 --actuator-model claude-opus-4.5 "Build API"

   # Custom energy weights
   perspt agent --energy-weights "2.0,1.0,3.0" "Fix type errors"

Init Command
------------

Initialize project configuration and memory files.

.. code-block:: text

   perspt init [OPTIONS]

   Options:
     --memory  Create PERSPT.md project memory file
     --rules   Create default Starlark policy rules (.perspt/rules.star)

Examples
~~~~~~~~

.. code-block:: bash

   # Initialize with memory file
   perspt init --memory

   # Initialize with policy rules
   perspt init --rules

   # Initialize both
   perspt init --memory --rules

Config Command
--------------

Manage configuration settings.

.. code-block:: text

   perspt config [OPTIONS]

   Options:
     --show           Show current configuration
     --set <KEY=VAL>  Set a configuration value
     --edit           Open in $EDITOR

Examples
~~~~~~~~

.. code-block:: bash

   # Show configuration
   perspt config --show

   # Set default model
   perspt config --set default_model=gpt-5.2

   # Edit in vim
   EDITOR=vim perspt config --edit

Ledger Command
--------------

Query and manage the Merkle ledger.

.. code-block:: text

   perspt ledger [OPTIONS]

   Options:
     --recent            Show recent commits
     --rollback <HASH>   Rollback to a specific commit
     --stats             Show ledger statistics

Examples
~~~~~~~~

.. code-block:: bash

   # View recent commits
   perspt ledger --recent

   # Rollback to commit
   perspt ledger --rollback abc123

   # Show statistics
   perspt ledger --stats

Status Command
--------------

Show current agent status.

.. code-block:: text

   perspt status

Displays:
- Current session ID
- Task in progress
- Energy levels
- Token usage

Abort Command
-------------

Abort the current agent session.

.. code-block:: text

   perspt abort [OPTIONS]

   Options:
     -f, --force  Force abort without confirmation

Resume Command
--------------

Resume a paused or crashed session.

.. code-block:: text

   perspt resume [SESSION_ID]

   Arguments:
     [SESSION_ID]  Session ID to resume (optional, uses latest if omitted)

Implementation
--------------

Command Routing
~~~~~~~~~~~~~~~

.. code-block:: rust

   #[derive(Subcommand)]
   enum Commands {
       Chat { model: Option<String> },
       Agent { task: String, ... },
       Init { memory: bool, rules: bool },
       Config { show: bool, set: Option<String>, edit: bool },
       Ledger { recent: bool, rollback: Option<String>, stats: bool },
       Status,
       Abort { force: bool },
       Resume { session_id: Option<String> },
   }

   #[tokio::main]
   async fn main() -> Result<()> {
       let cli = Cli::parse();
       
       match cli.command {
           None | Some(Commands::Chat { .. }) => commands::chat::run().await,
           Some(Commands::Agent { .. }) => commands::agent::run(...).await,
           Some(Commands::Init { .. }) => commands::init::run(...).await,
           // etc.
       }
   }

Source Code
-----------

- ``crates/perspt-cli/src/main.rs`` - CLI definition and routing
- ``crates/perspt-cli/src/commands/`` - Command implementations:
  - ``chat.rs``
  - ``agent.rs``
  - ``init.rs``
  - ``config.rs``
  - ``ledger.rs``
  - ``status.rs``
  - ``abort.rs``
  - ``resume.rs``
