Introduction to Perspt
======================

Perspt is an interface designed to bring the capabilities of Large Language Models (LLMs) directly and reliably into your terminal. Rather than treating artificial intelligence as a disconnected web service, Perspt integrates LLMs into your daily development workflow, offering three distinct interaction surfaces: Simple Chat, TUI, and Agent.

Whether you need to quickly ask a question using command-line pipelines, converse interactively in a rich terminal interface, or delegate complex multi-file engineering tasks to a team of specialized agents, Perspt provides a unified, structured, and stable environment for developer-AI collaboration.

The Three Surfaces of Perspt
----------------------------

Perspt operates across three primary surfaces, each designed for a different type of developer workflow:

1. **Simple Chat Mode (``simple-chat``)**:
   A lightweight, streamable command-line interface. It is designed to converse with an LLM and integrate seamlessly with Unix bash pipelines. You can quickly pipe files, logs, or command outputs directly into the model to request a response, and redirect the LLM's output to other command-line utilities. This makes it an ideal tool for shell scripting, fast code analysis, and automated diagnostics.

2. **Interactive TUI Mode (``chat``)**:
   A rich, full-screen Terminal User Interface (TUI) built on ``ratatui``. It provides an interactive conversational interface with the LLM right on your terminal. The TUI supports real-time streaming, markdown rendering with syntax highlighting, inline LaTeX math equations, keyboard-driven navigation, and prompt history. It allows you to explore ideas, ask code questions, and interactively write code without leaving your terminal workspace.

3. **Agent Mode (``agent``)**:
   A collaborative multi-agent system composed of specialized virtual actors (the Architect, Actuator, Verifier, and Speculator) that work together to solve complex software engineering and coding jobs. It models development tasks as state graphs and uses the **Stabilized Recursive Barrier Network (SRBN)** framework to ensure reliability and workflow stability. By integrating with local tests, compilers, and linters, Agent Mode automatically detects, corrects, and verifies code updates, ensuring the repository moves systematically toward a clean, working state.

The Problem of Guessing (Reliability via Verification)
------------------------------------------------------

To understand why Perspt is designed this way, consider the challenge of writing complex code.
Every time a language model suggests code, it is making a guess. Because language models are non-deterministic and lack internal verification loops, even a tiny error in a single guess can propagate across files. Left to run on its own without external guidance, an agentic system will quickly drift away from a working codebase, culminating in compilation failures and broken tests.

In the spirit of Leslie Lamport’s work on distributed systems, where consensus must be reached among potentially faulty or unpredictable nodes, Perspt does not trust the model to self-verify. Instead, it wraps the model in a deterministic control loop. The system:

- Validates every proposed change using local type-checkers, compilers, and test suites.
- Measures errors as a scalar value (the Lyapunov energy).
- Feeds specific compiler and test diagnostics back to the LLM to guide corrections.
- Commits changes to the code repository only when the workspace is stable and error-free.

Perspt implements this discipline through the SRBN orchestrator.


Core System Features
--------------------

We summarize the capabilities of the system in terms of their operational invariants:

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Invariant
     - Operational Specification
   * - **Multi-Provider Hub**
     - Establishes unified client communication with OpenAI, Anthropic, Gemini, Groq, Cohere, XAI, DeepSeek, AWS Bedrock, Vertex AI, and local Ollama instances.
   * - **LSP Verification**
     - Connects directly to language-server protocols (including ``rust-analyzer``, ``pyright``, and ``typescript-language-server``) to extract syntactic diagnostics.
   * - **Test Automation**
     - Integrates with local test runtimes (such as ``pytest`` and Cargo) to compute logic error metrics.
   * - **Role Specialization**
     - Segregates agent processes into four distinct reasoning tiers: Architect (planning), Actuator (generation), Verifier (measurement), and Speculator (lookahead).
   * - **Policy Sandbox**
     - Restricts process execution via a Starlark-based policy engine, preventing command execution or file mutation outside the workspace boundaries.
   * - **Resource Budgeting**
     - Halts execution when the token count, USD cost, or round count exceeds strict safety bounds.
   * - **Terminal Interface (TUI)**
     - Renders unified diff views, hierarchical task trees, and live telemetry dashboards within a terminal-based interface.
   * - **Web Telemetry**
     - Streams real-time heatmaps, state-graph topologies, and LLM diagnostics to a browser-based visualization dashboard.

Theoretical Foundation of SRBN
------------------------------

The Stabilized Recursive Barrier Network (SRBN) is derived from the *Stability is All You Need* paper series. The framework treats LLM-based software modification as a trajectory stabilization problem. By wrapping the generative model inside a deterministic verification barrier, we convert an unconstrained random walk into a guided descent toward a stable state.

Key theoretical results from the papers include:

- **Input-to-State Stability (ISS)**: Mathematical proofs demonstrating that bounded reasoning errors from the model result in bounded deviations in the system state.
- **Flow Matching Barriers**: Projection operators that take a diverging, erroneous state trajectory and map it back onto the safe manifold.
- **Logarithmic Reliability Scaling**: A prediction that the expected number of correction attempts scales logarithmically (:math:`O(\log N)`) with the size of the target codebase :math:`N`, rather than decaying exponentially as in unbarriered systems.

The chart below illustrates this conceptual scaling difference:

.. plot::
   :caption: Conceptual error accumulation: unbarriered random walk vs. SRBN logarithmic bound.

   import numpy as np
   import matplotlib.pyplot as plt

   N = np.arange(1, 101)
   naive = 0.05 * N                 # O(N): errors accumulate every step
   srbn = 0.05 * np.log(N + 1)      # O(log N): barrier-gated acceptance

   fig, ax = plt.subplots()
   ax.plot(N, naive, label=r'Unbarriered agent  $O(N)$', color='#E53935', lw=2)
   ax.plot(N, srbn, label=r'SRBN prediction  $O(\log N)$', color='#1E88E5', lw=2)
   ax.set_xlabel('Autonomous steps $N$')
   ax.set_ylabel('Expected uncorrected errors')
   ax.set_title('Reliability scaling: accumulation vs. logarithmic bound')
   ax.legend()

The operational execution of this control loop is represented as follows:

.. graphviz::
   :align: center
   :caption: SRBN Control Flow

   digraph srbn {
       rankdir=LR;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10];

       detect [label="Detect\n(Tooling)", fillcolor="#E0F7FA"];
       plan [label="Plan\n(Architect)", fillcolor="#E8F5E9"];
       gen [label="Generate\n(Actuator)", fillcolor="#FFF3E0"];
       verify [label="Verify\n(LSP+Tests)", fillcolor="#F3E5F5"];
       check [label="V(x) <= e?", shape=diamond, fillcolor="#FFECB3"];
       sheaf [label="Sheaf\n(Consistency)", fillcolor="#E8EAF6"];
       commit [label="Commit\n(Ledger)", fillcolor="#C8E6C9"];

       detect -> plan;
       plan -> gen;
       gen -> verify;
       verify -> check;
       check -> gen [label="retry", style=dashed, color="#E53935"];
       check -> sheaf [label="stable"];
       sheaf -> commit;
   }

System Architecture
-------------------

The Perspt system is constructed as a workspace of twelve crates. The dependencies and responsibilities are partitioned as follows:

- **The User Interface**: Handled by ``perspt-cli`` (command parsing) and ``perspt-tui`` (terminal rendering).
- **The Core Engine**: Handled by ``perspt-core`` (provider abstractions and types), ``perspt-agent`` (the orchestration loop), and ``perspt-store`` (session databases).
- **The Security Envelope**: Handled by ``perspt-policy`` (Starlark checks) and ``perspt-sandbox`` (isolated execution environments).
- **The Monitoring Plane**: Handled by ``perspt-dashboard`` (web interface).
- **The Reusable SDK**: Handled by ``perspt-sdk``, ``perspt-coding``, and ``perspt-research``.

The separation between ``perspt-sdk`` and domain packages (like ``perspt-coding``) ensures that the mathematical control loop remains independent of specific execution environments. The same SDK stabilizes both code repositories and academic research projects.

The current implementation is in a transition state: the orchestrator executes the main loop using its legacy scheduler and node graph, while running the SDK's measured acceptance gate in parallel to collect telemetry and prepare for full SDK integration.

System Command Index
--------------------

The application interface is accessed via the following commands:

.. list-table::
   :header-rows: 1
   :widths: 15 45 40

   * - Command
     - Function
     - Invocation Example
   * - ``chat``
     - Launches the interactive terminal UI (default).
     - ``perspt chat --model gemini-3.1-pro``
   * - ``simple-chat``
     - Launches a streamable CLI interface without terminal styling.
     - ``perspt simple-chat``
   * - ``agent``
     - Launches autonomous agent mode.
     - ``perspt agent "Create a parser in Rust"``
   * - ``init``
     - Instantiates memory files and policy rule structures.
     - ``perspt init --memory --rules``
   * - ``config``
     - Displays or writes configuration values.
     - ``perspt config --show``
   * - ``ledger``
     - Queries the Merkle state history.
     - ``perspt ledger --recent``
   * - ``status``
     - Returns session metrics and active energy values.
     - ``perspt status``
   * - ``abort``
     - Signals an active agent session to terminate.
     - ``perspt abort``
   * - ``resume``
     - Resumes the most recently interrupted session.
     - ``perspt resume --last``
   * - ``logs``
     - Inspects LLM communication logs.
     - ``perspt logs --tui``
   * - ``dashboard``
     - Launches the web monitoring server.
     - ``perspt dashboard``

Supported Oracles and Providers
-------------------------------

The client library supports direct integration with the following external providers:

.. list-table::
   :header-rows: 1
   :widths: 20 30 50

   * - Provider
     - Target Env Variable
     - Reference Models
   * - **OpenAI**
     - ``OPENAI_API_KEY``
     - gpt-5.5, gpt-5-mini
   * - **Anthropic**
     - ``ANTHROPIC_API_KEY``
     - claude-fable, opus-4.8, claude-3-5-sonnet
   * - **Google Gemini**
     - ``GEMINI_API_KEY``
     - gemini-3.5-flash, gemini-3.1-pro
   * - **Google Vertex AI**
     - ``VERTEX_API_KEY``
     - vertex::gemini-3.5-flash, vertex::gemini-3.1-pro
   * - **Groq**
     - ``GROQ_API_KEY``
     - llama-4-70b
   * - **Cohere**
     - ``COHERE_API_KEY``
     - command-r7
   * - **XAI**
     - ``XAI_API_KEY``
     - grok-4
   * - **DeepSeek**
     - ``DEEPSEEK_API_KEY``
     - deepseek-v4, deepseek-coder-v4
   * - **Ollama**
     - None
     - llama4, mistral

System Philosophy
-----------------

.. epigraph::

   | *The keyboard hums, the screen aglow,*
   | *AI's wisdom, a steady flow.*
   | *Through SRBN's loop, stability we find,*
   | *Code that works, refined and aligned.*

  -- The Perspt Manifesto

Perspt is built upon five design principles:

- **Accessibility**: A single command must establish communication with any supported model.
- **Verification**: Code is not committed until it passes deterministic verification checks.
- **Determinism**: Transitions and rollback events must be event-sourced to ensure reproducibility.
- **Safety**: Execution limits must be enforced via sandboxes and policy modules.
- **Modularity**: The engine components must remain separated into reusable crates.
