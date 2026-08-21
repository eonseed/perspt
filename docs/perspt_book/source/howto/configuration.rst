.. _howto-configuration:

Configuration
=============

Config File Location
--------------------

Perspt searches for configuration in this order:

1. ``--config <path>`` (CLI flag)
2. ``~/.config/perspt/config.toml`` (Linux),
   ``~/Library/Application Support/perspt/config.toml`` (macOS),
   ``%APPDATA%\perspt\config.toml`` (Windows)
3. Environment variables
4. Auto-detection

Config File Format
------------------

The file is TOML. All fields are optional. ``provider`` accepts the aliases
``provider_type`` and ``default_provider``; ``model`` accepts the alias
``default_model``.

.. code-block:: toml

   provider = "gemini"
   model = "gemini-3.5-flash"
   api_key = "AIza..."

   # Optional endpoint override for OpenAI-compatible / local / proxy servers
   # base_url = "http://localhost:8000/v1"

   # Optional per-role routes for `perspt agent`; values stay fully
   # qualified as provider::model
   # [models]
   # architect = "openai::gpt-5.5"
   # actuator = "openai::gpt-5-mini"
   # verifier = "openai::gpt-5-mini"
   # speculator = "openai::gpt-5-mini"
   # adjudicator = "openai::gpt-5.5"

Agent Runtime Sections
----------------------

``perspt agent`` reads further optional TOML tables, each validated at
startup.

``[providers.<id>]`` lets several provider credentials coexist in one
process. ``api_key_env`` names an environment variable; Vertex entries may
omit a static key and use the per-request ADC token resolver:

.. code-block:: toml

   [providers.anthropic]
   api_key_env = "ANTHROPIC_API_KEY"

   [providers.local]
   adapter = "ollama"
   base_url = "http://localhost:11434"

``[exploration]`` bounds the PSP-10 search forest: sequential eager
branches with a hard branch cap of 3. The former ``[ensemble]`` section was
removed by PSP-10 - a present block fails validation with a migration error
pointing here:

.. code-block:: toml

   [exploration]
   initial_branches = 1
   max_branches = 3
   distinct_family = true

``[[external_tools]]`` declares shared latest-only MCP 2026-07-28 servers.
The official Rust SDK supplies tools, resources, prompts, completions, roots,
sampling, elicitation, subscriptions, MRTR, and tasks. Secret values are
never stored here: environment maps contain destination names and source
variable names. Omitted ``modes`` means agent-only; chat must be explicit.
Every allowed remote tool needs a local policy, including a schema-valid
footprint:

.. code-block:: toml

   [[external_tools]]
   id = "docs"
   transport = "stdio"
   command = ["company-docs-mcp", "--stdio"]
   modes = ["agent", "chat"]
   env_from_env = { DOCS_TOKEN = "COMPANY_DOCS_TOKEN" }
   roots = [{ uri = "file:///absolute/path/to/project", name = "Project" }]
   sampling = true
   max_sampling_tokens = 4096
   elicitation = true
   subscriptions = true
   tasks = true
   max_task_wait_ms = 300000

   [external_tools.tools.search]
   effect = "search"
   risk = "low"
   footprint = { selectors = [{ kind = "scoped_argument", family = "company-docs", field = "query", access = "read" }] }

The policy key must equal the remote tool name and its ``field`` must exist in
the advertised input schema. In chat, ``/mcp`` explains failures and lists the
admitted namespaced operations. Older protocol versions have no fallback. See
:doc:`../user-guide/mcp` for both transports, every capability and bound,
agent/chat behavior, clipboard paste, and security guidance.

``[prompts]`` pins external prompt replacement bundle directories and the
paired activation bounds (Gate AE):

.. code-block:: toml

   [prompts]
   bundles = ["./prompt-bundles/coding"]
   activation_min_tasks = 30

``[context]`` sets reserves and working-set bounds for the paged resident
context; every configured reserve must be positive:

.. code-block:: toml

   [context]
   working_set_turns = 8
   output_reserve_tokens = 4096

``[verification]`` sets the test-evidence policy, optional ``format`` sensor,
process-isolation fallback, and per-stage timeouts. ``evolving`` is the iterative-development default;
``backward-compatible`` additionally runs recognized historical test files;
``external-oracle`` additionally runs a protected overlay configured under
``[verification.external_oracle]``:

.. code-block:: toml

   [verification]
   test_policy = "evolving"
   allow_unisolated = false
   require_format = true
   stage_timeout_secs = 180

Keep ``allow_unisolated`` false for fail-closed execution. Native Windows users
may set it true to run compilers, tests, inspection commands, and LSP servers
without an OS sandbox; Perspt retains its agent-level gates but child processes
have the host user's authority.

Environment Variables
---------------------

.. list-table::
   :header-rows: 1
   :widths: 35 20 45

   * - Variable
     - Provider
     - Priority
   * - ``VERTEX_PROJECT_ID``
     - Vertex AI
     - Highest
   * - ``GEMINI_API_KEY``
     - Gemini
     - 2
   * - ``OPENAI_API_KEY``
     - OpenAI
     - 3
   * - ``ANTHROPIC_API_KEY``
     - Anthropic
     - 4
   * - ``GROQ_API_KEY``
     - Groq
     - 5
   * - ``COHERE_API_KEY``
     - Cohere
     - 6
   * - ``XAI_API_KEY``
     - xAI
     - 7
   * - ``DEEPSEEK_API_KEY``
     - DeepSeek
     - 8
   * - *(none)*
     - Ollama
     - Fallback

.. note::

   When multiple keys are set, the highest-priority provider is used. Override
   with the ``provider`` field in the config or by setting only the key you
   want detected.


CLI Flag and Config Override
----------------------------

Manage the config file from the CLI:

.. code-block:: bash

   # Show the effective config (api_key masked)
   perspt config --show

   # Set values (structured TOML write)
   perspt config --set provider=openai
   perspt config --set default_model=gpt-5.5

   # Edit in $EDITOR
   perspt config --edit

   # Override the model per run
   perspt chat --model gpt-5.5

   # Use a specific config file
   perspt --config /path/to/config.toml chat

Logging Configuration
---------------------

.. code-block:: bash

   # Default: error-level logging only (avoids TUI noise)
   perspt

   # Enable debug logging with RUST_LOG
   RUST_LOG=debug perspt simple-chat

   # Audit a finished agent run (deterministic, credential-free replay)
   perspt replay <SESSION_ID>
