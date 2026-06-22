.. _stability-agent-mode:

Stability Papers in Agent Mode
==============================

The execution of autonomous agents in Perspt is governed by control-theoretic stability contracts derived from the *Stability is All You Need* paper series. We do not assume that a language model's proposal is correct. Instead, the agent system measures the proposed state, determines its residual errors, and admits changes only when they satisfy a formal convergence constraint.

We formulate this stabilization process using three distinct contracts.

The Three Contracts
-------------------

Let a session consist of a sequence of proposed states :math:`x_1, x_2, \dots` and a corresponding sequence of accepted states :math:`s_1, s_2, \dots`.

Paper I: The Single-Agent Stability Contract
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Paper I (*Stability is All You Need: Lyapunov-Guided Hierarchies for Long-Horizon LLM Reliability*) establishes the concept of a state-space manifold and the Lyapunov energy function. 

Let :math:`x` be a candidate state. The system computes a vector of residual errors :math:`r(x)`. We define the Lyapunov energy :math:`V(x)` as a measure of the distance from :math:`x` to the verified manifold :math:`V(x) = 0`. The single-agent contract asserts that the state converges if each step reduces :math:`V(x)`.

In the Perspt runtime, the candidate state corresponds to a modification proposed by the actuator model. The orchestrator validates the proposal using domain-specific sensors to calculate the energy.

Paper II: The Harness Contract
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Paper II (*Stability is All You Need II: SRBN-Control Beyond Harness Engineering*) describes the measurement harness. The harness must observe every candidate, partition them into observed and accepted trajectories, and guarantee that the system terminates with a formal certificate of remaining errors if convergence is not achieved.

In the Perspt runtime, this contract is implemented by the tracking engine. Every verification run, retry attempt, and error message is persisted in the ledger, culminating in either a committed state or an explicit escalation report.

Paper III: The Platform Contract
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Paper III (*Stability is All You Need III: The SRBN Platform Contract*) lifts the single-agent harness to a multi-domain platform. The platform must isolate stochastic proposals from deterministic side-effects. It mandates that models propose actions, while a deterministic capability kernel enforces security bounds and records budgets.

In the Perspt runtime, this is implemented via the SDK-first separation. The core SRBN engine (in ``perspt-sdk``) manages scheduling and gating, while domain packages (such as ``perspt-coding``) define domain-specific sensors and capabilities.

The Byzantine Generals Metaphor
-------------------------------

To illustrate the stabilization protocol, we map the closed-loop convergence system to the Byzantine Generals Problem formulation (Lamport, Shostak, and Pease, 1982).

Let the Actuator agent be a commanding general whose loyalty is unknown; the commander's objective is to propose a series of operations (file modifications, deletions, or command executions) to reach a stable repository state. Let the verification sensors (the compiler, the type-checker, the linter, and the test oracle) be the loyal lieutenants. 

The protocol proceeds in rounds:

1. **Proposal**: The commander (Actuator) issues a proposed workspace change.
2. **Observation**: The loyal lieutenants (Sensors) independently inspect the proposal and report their findings as a vector of residual events.
3. **Consensus**: The gatekeeper (Orchestrator) aggregates the lieutenants' reports using the quadratic energy function. If the loyal lieutenants consensus reveals no critical residual errors (i.e., :math:`V(x) \le \varepsilon`), the proposal is committed.
4. **Correction or Escalation**: If the energy exceeds the threshold, the lieutenants' specific error reports are compiled into a feedback message. The commander is instructed to repair the state. If the commander fails to reach stability within the allocated round budget, the lieutenants reject further proposals and execute an escalation protocol to alert the operator.

The Energy Formulation
----------------------

Let :math:`E` be the set of active sensors. Each sensor :math:`e \in E` evaluates the candidate state :math:`x` and emits a residual magnitude :math:`r_e(x) \geq 0`. 

The Lyapunov energy :math:`V(x)` is the weighted sum of the squared residuals:

.. math::

   V(x) = \sum_{e \in E} w_e \, \lVert r_e(x) \rVert^2, \qquad w_e > 0

We group the individual residuals into five component rollups:

- **Syntactic energy** (:math:`V_{\text{syn}}`): Derived from compiler syntax errors and language server diagnostics.
- **Structural energy** (:math:`V_{\text{str}}`): Derived from interface contract violations.
- **Logical energy** (:math:`V_{\text{log}}`): Derived from test suite failures.
- **Bootstrap energy** (:math:`V_{\text{boot}}`): Derived from initialization and build environment failures.
- **Sheaf energy** (:math:`V_{\text{sheaf}}`): Derived from cross-node import and dependency contradictions.

The total energy is the sum of these component rollups:

.. math::

   V(x) = V_{\text{syn}} + V_{\text{str}} + V_{\text{log}} + V_{\text{boot}} + V_{\text{sheaf}}

A candidate state :math:`x` is admitted to the accepted trajectory if and only if it satisfies the gating condition:

.. math::

   \operatorname{accept}(x) \iff V(x) \leq \varepsilon \quad \lor \quad V(x) < V(x_{\text{best}}) - \rho_{\text{gate}}

where :math:`\varepsilon` is the convergence threshold (default :math:`0.10`), :math:`x_{\text{best}}` is the best previously accepted state in the current node generation, and :math:`\rho_{\text{gate}}` is the minimum required descent step.

Observed vs. Accepted Trajectories
----------------------------------

We maintain a strict boundary between the actions the model proposes and the actions the system commits.

Let :math:`T_{\text{obs}}` be the set of all observed candidate states:

.. math::

   T_{\text{obs}} = \{ x_t \mid t \ge 1 \}

Let :math:`T_{\text{acc}}` be the set of accepted states:

.. math::

   T_{\text{acc}} = \{ x \in T_{\text{obs}} \mid \operatorname{accept}(x) = \text{true} \}

The Merkle ledger commits only elements of :math:`T_{\text{acc}}`. Unaccepted proposals in :math:`T_{\text{obs}} \setminus T_{\text{acc}}` are recorded as telemetry for correction feedback, but they never modify the workspace files.

The GoalPresence Verification Sensor
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

A primary check performed by the structural sensor (lieutenant) is the validation of required symbol presence. Let :math:`\mathcal{S}_{\text{req}}` be the set of symbols (functions, structures, classes, or modules) that the node is required to implement, as declared by its behavioral contract interface signature and goal text. Let :math:`\mathcal{S}_{\text{obs}}` be the set of symbols actually defined in the generated source code.

The goal-presence sensor computes the missing required symbols set :math:`\mathcal{M}`:

.. math::

   \mathcal{M} = \mathcal{S}_{\text{req}} \setminus \mathcal{S}_{\text{obs}}

If :math:`\mathcal{M} \neq \emptyset`, the sensor emits a blocking residual of class ``SymbolMismatch``:

.. math::

   r_{\text{symbol}}(x) = \lvert \mathcal{M} \rvert

This forces the structural energy component :math:`V_{\text{str}}` to remain high, preventing the state from satisfying the acceptance gate until all required symbols are present in the workspace.

Operational Mechanisms
----------------------

The platform achieves stabilization through five mechanisms:

1. **Closed-Loop Scheduler**: Re-evaluates the ready state of the graph dynamically. If a node fails to converge, the scheduler registers a repair action as a graph revision event (e.g., splitting a node or inserting an interface node).
2. **Capability Filtering**: All proposals must pass through the admissibility kernel. A proposal requesting a file edit or command run must possess a valid capability token.
3. **Structured Prompt Compilation**: When a proposal is rejected, the system compiles the residual diagnostics into a correction prompt.
4. **Event-Sourced Ledger**: Every transaction (proposals, rejections, commits, rollbacks) is written to a Merkle ledger to enable complete session replay.
5. **Domain Abstraction**: The core engine processes abstract residuals and energy models. Domain-specific behavior is relegated to domain packages.