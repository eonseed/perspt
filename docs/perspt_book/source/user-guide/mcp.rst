.. _user-guide-mcp:

Model Context Protocol (MCP)
============================

Perspt is an optional MCP client for the governed coding agent and the
interactive chat TUI. The implementation uses the official Rust MCP SDK and
speaks **MCP 2026-07-28 only**. It starts with ``server/discover`` and does not
fall back to ``initialize`` or an older protocol version. A server that does
not advertise 2026-07-28 is rejected.

The client supports stdio and stateless Streamable HTTP, including:

* tools, resources, resource templates, prompts, and completions;
* explicit roots disclosure;
* opt-in model sampling and user elicitation;
* progress, logging, cancellation, catalog-change, resource-update, task, and
  custom notifications;
* ``subscriptions/listen`` with list-change and resource filters;
* multi-round tool results and the tasks extension for long-running calls.

MCP is absent when no ``[[external_tools]]`` table is configured. It does not
replace Perspt's coding-domain tools, planner, verifier, candidate workspace,
or acceptance gate.

Authority and Trust
-------------------

A server description, schema, prompt, resource, notification, or result is
untrusted input. A server cannot grant itself authority. Each remote tool must
match a user-owned policy assigning its effect, risk, and scheduling
footprint. Undeclared tools fail closed. Agent calls still pass the capability
kernel and are write-ahead bracketed; replay uses the recorded result and
never reconnects.

Resources and prompts are exposed through namespaced, locally defined
read-only operations only when the server advertises the capability:

.. code-block:: text

   mcp.<server>._perspt_resources_list
   mcp.<server>._perspt_resource_templates_list
   mcp.<server>._perspt_resource_read
   mcp.<server>._perspt_prompts_list
   mcp.<server>._perspt_prompt_get
   mcp.<server>._perspt_complete

Their output is returned as untrusted data and is never silently inserted
into the system prompt. Remote names beginning with ``_perspt_`` are reserved
and rejected, preventing collisions with these client operations.
``_perspt_prompt_get`` accepts optional, equally sized ``argument_names`` and
``argument_values`` string arrays; entries at the same index form one MCP
prompt argument. Each array is capped at 64 entries.

Quick Setup
-----------

This stdio example enables independent agent and chat lifecycles, discloses
one root, permits sampling and elicitation, and subscribes to one resource:

.. code-block:: toml

   [[external_tools]]
   id = "docs"
   transport = "stdio"
   command = ["company-docs-mcp", "--stdio"]
   modes = ["agent", "chat"]
   timeout_ms = 30000
   max_result_bytes = 1048576
   max_stderr_bytes = 65536
   max_task_wait_ms = 300000
   env_from_env = { DOCS_TOKEN = "COMPANY_DOCS_TOKEN" }

   roots = [{ uri = "file:///absolute/path/to/project", name = "Project" }]
   sampling = true
   max_sampling_tokens = 4096
   elicitation = true
   subscriptions = true
   resource_subscriptions = ["file:///absolute/path/to/project/guide.md"]
   tasks = true

   [external_tools.tools.search]
   effect = "search"
   risk = "low"
   footprint = { selectors = [{ kind = "scoped_argument", family = "company-docs", field = "query", access = "read" }] }

The policy key must exactly match the remote name. A footprint field must
exist in the advertised input schema. After admission the model-facing tool
is ``mcp.docs.search``.

For Streamable HTTP use these transport fields:

.. code-block:: toml

   [[external_tools]]
   id = "records"
   transport = "streamable_http"
   url = "https://tools.example.test/mcp"
   modes = ["agent", "chat"]
   headers_from_env = { Authorization = "RECORDS_AUTHORIZATION" }

   [external_tools.tools.lookup]
   effect = "data_read"
   risk = "low"
   footprint = { selectors = [{ kind = "scoped_argument", family = "records", field = "id", access = "read" }] }

Set ``RECORDS_AUTHORIZATION`` to the complete server value, for example
``Bearer ...``. Non-loopback endpoints require HTTPS and redirects are
disabled. Streamable HTTP is stateless under 2026-07-28; Perspt does not
recover by negotiating a legacy session.

Chat TUI
--------

Run ``perspt --config ./config.toml chat`` and type ``/mcp``. It reports
discovery failures, policy rejections, and admitted operations. ``/help``
includes MCP and elicitation commands. The model calls tools automatically;
``/mcp`` performs no remote call.

Normal chat starts without discovery or policy messages in the conversation.
When the model selects an admitted operation, the TUI shows one transient,
human-readable activity label and then the final answer. Raw MCP lifecycle
events, tool arguments, results, subscription notifications, and server logs
stay out of both the conversation and the reasoning panel. Use ``/mcp`` when
you explicitly want discovery and admission diagnostics. **Ctrl+R** remains
the reasoning control and displays only reasoning emitted by the selected
model; tool-aware Qwen turns use the same reasoning stream.

Chat has a fixed read-only authority ceiling. Mutating, shell, network-fetch,
dependency, graph, and policy effects are rejected even when the same tool is
admissible in the governed agent. ``perspt simple-chat`` intentionally creates
no MCP lifecycle.

For form or URL elicitation the input remains active and the TUI shows the
request as JSON. Respond explicitly with:

.. code-block:: text

   /mcp accept {"field":"value"}
   /mcp accept
   /mcp decline
   /mcp cancel

Perspt does not open an elicitation URL automatically. Form responses are JSON
objects. Perspt advertises client-side schema validation as disabled, leaving
the server responsible for validating the requested schema.

Terminal bracketed paste is enabled in both chat input paths. Normal terminal
paste shortcuts (Command-V, Ctrl-Shift-V, or Shift-Insert, depending on the
terminal) insert Unicode and multiline clipboard text at the cursor. The mode
is disabled again on TUI exit.

Agent Behavior
--------------

Agent servers are discovered when a session assembles its node catalog.
External schemas use the ordinary deferred tool-search path. Proposals,
arguments, effects, grants, execution, results, and uncertain completion are
governed like the rest of the coding tool plane.

Sampling uses the already selected local route. MCP model preferences cannot
redirect credentials or choose an unconfigured provider. Perspt forwards
``maxTokens``, temperature, stop sequences, tools, and tool choice. It
advertises tool-aware sampling but not ``includeContext``; requests for
implicit MCP-server context are rejected.

The command-line agent has no interactive MCP form surface, so an enabled
elicitation request receives ``decline`` immediately. Chat is interactive.
SDK hosts may install their own ``McpElicitationProvider``.

Subscriptions and Dynamic Catalogs
----------------------------------

With ``subscriptions = true`` (the default), Perspt intersects its filter with
server capabilities before opening ``subscriptions/listen``. Tool-list changes
invalidate the SDK cache; the shared runtime removes old bindings and repeats
local admission before a chat model turn. A removed tool cannot remain
callable. Resource and prompt changes, progress, logging, tasks, cancellation,
and custom extension notifications are typed ``McpServerEvent`` values for
product display or SDK consumption.

``resource_subscriptions`` contains exact URIs and should be narrow. An empty
list still permits supported tools/prompts/resources list-change filters.

Multi-Round Results and Tasks
-----------------------------

Tool calls accept complete results, bounded multi-round input-required
responses, or asynchronous task handles. Inputs may request sampling,
elicitation, or roots only when locally enabled; unknown kinds fail closed.

Every wire request uses ``timeout_ms``. ``max_task_wait_ms`` is an overall
deadline across task polling and input rounds (default five minutes), and poll
hints are clamped. A failed, cancelled, malformed, or expired task becomes an
uncertain external completion for agent reconciliation.

Configuration Reference
-----------------------

.. list-table:: ``[[external_tools]]`` fields
   :header-rows: 1
   :widths: 27 73

   * - Field
     - Meaning
   * - ``id``
     - Required ASCII id used in ``mcp.<id>.<operation>``.
   * - ``transport``
     - Exactly ``stdio`` or ``streamable_http``.
   * - ``command`` / ``url``
     - Direct argv for stdio or HTTP endpoint; mutually exclusive.
   * - ``modes``
     - ``["agent"]``, ``["chat"]``, or both. Omission means agent only.
   * - ``env_from_env``
     - Stdio child variable to source-variable mapping. Children otherwise
       receive only ``PATH`` and Windows ``SystemRoot``.
   * - ``headers_from_env``
     - HTTP header to source-variable mapping; values stay out of TOML.
   * - ``timeout_ms``
     - Per-request and discovery deadline; default 30000.
   * - ``max_result_bytes``
     - Maximum accepted serialized result and raw SSE event; default 1 MiB.
   * - ``max_stderr_bytes``
     - Rolling stdio stderr tail; default 64 KiB.
   * - ``roots``
     - Explicit absolute ``file://`` roots. Empty means unadvertised.
   * - ``sampling`` / ``max_sampling_tokens``
     - Opt in to server model calls and cap one request (default 4096).
   * - ``elicitation``
     - Opt in to form/URL elicitation; interactive in chat, declined by the
       non-interactive agent.
   * - ``subscriptions`` / ``resource_subscriptions``
     - Enable the listen stream (default true) and exact resource filters.
   * - ``tasks`` / ``max_task_wait_ms``
     - Enable task/MRTR handling (default true) and its overall deadline
       (default 300000).
   * - ``tools``
     - Local policy tables keyed by exact remote tool name.

Each policy accepts ``effect``, ``risk``, ``footprint``, and optional
``proposal_bindings``. Omitting effect or footprint becomes a high-risk opaque
shell declaration and is not admitted by default.

Security Notes
--------------

A configured stdio command is user-selected code, and an HTTP server acts
outside the candidate workspace. Admission constrains calls but does not
sandbox the server. Trust its implementation/operator and grant the narrowest
policy.

Result objects are size-checked after protocol decoding and SSE events before
decoding. Stderr capture, subscription filters, MRTR rounds, task duration,
sampling tokens, and chat tool rounds are bounded. The official SDK owns
framing, correlation, cancellation, and dispatch; Perspt owns transport
security, capabilities, admission, product interaction, and replay policy.

Troubleshooting
---------------

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Symptom
     - Check
   * - ``/mcp`` says disabled
     - Add ``"chat"`` to ``modes`` and restart chat.
   * - Protocol is incompatible
     - Upgrade the server to 2026-07-28; there is no legacy fallback.
   * - No remote tools are admitted
     - Add exact policies and schema-valid footprints. Resource/prompt
       operations may still appear when advertised.
   * - Stdio program or credential is missing
     - Verify direct argv and map variables with ``env_from_env``.
   * - HTTP is refused
     - Use HTTPS outside localhost and check header source variables.
   * - Sampling is rejected
     - Enable it, stay under the token cap, and omit unadvertised context.
   * - Elicitation is declined in agent mode
     - Use chat or install an SDK host provider.
   * - A task times out
     - Increase the overall deadline deliberately or fix the server.
