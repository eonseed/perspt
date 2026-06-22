Introduction to Perspt
======================

Perspt is a system designed to address the problem of reliable communication and execution with non-deterministic Large Language Models (LLMs). The system provides two modes of operation:

1. **A Diagnostic Interface**: A terminal interface for communicating with various LLM providers, enabling developers to evaluate and compare model responses.
2. **A Stabilized Execution Engine**: An implementation of the Stabilized Recursive Barrier Network (SRBN) framework. The engine models multi-file development tasks as state graphs and guides generation toward verified manifolds using a control-theoretic Lyapunov energy gate.

The Problem of Guessing
-----------------------

A language model operates by proposing sequences of tokens. In complex tasks, such as multi-file software construction, each proposed change is a guess. Because these models are non-deterministic and lack internal verification loops, errors propagate. Left unchecked, a sequence of guesses drifts away from a working state, eventually resulting in system failure.

To prevent this drift, we must not rely on the model's self-assessment. We require a system that:

- Measures the correctness of each proposed modification using external, deterministic tools (such as compilers, type-checkers, and test suites).
- Quantifies the remaining errors as a non-negative scalar value (the Lyapunov energy).
- Generates targeted feedback from these error vectors to guide subsequent model attempts.
- Commits changes to the persistent workspace only when the energy converges to zero or falls within an acceptable tolerance.

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
     - ``perspt chat --model gemini-pro-latest``
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
     - gpt-4o, o3-mini, o1-preview
   * - **Anthropic**
     - ``ANTHROPIC_API_KEY``
     - claude-3-5-sonnet, claude-3-5-opus
   * - **Google Gemini**
     - ``GEMINI_API_KEY``
     - gemini-2.5-pro, gemini-2.5-flash
   * - **Groq**
     - ``GROQ_API_KEY``
     - llama-3.3-70b (optimized for low-latency inference)
   * - **Cohere**
     - ``COHERE_API_KEY``
     - command-r-plus
   * - **XAI**
     - ``XAI_API_KEY``
     - grok-2
   * - **DeepSeek**
     - ``DEEPSEEK_API_KEY``
     - deepseek-chat, deepseek-coder
   * - **Ollama**
     - None (local port)
     - Local model execution (no external network required)

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
