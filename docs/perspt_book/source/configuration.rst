Configuration Guide
===================

Perspt supports zero-config auto-detection, environment variables, a TOML config
file, and command-line flags. They are applied in this priority order (highest first):

1. **Command-line arguments**
2. **Configuration file** (``config.toml``)
3. **Environment variables**
4. **Auto provider detection**
5. **Built-in defaults**

Automatic Provider Detection
-----------------------------

Set any supported API key environment variable and run ``perspt`` with no arguments:

.. list-table::
   :header-rows: 1
   :widths: 10 30 30 30

   * - Priority
     - Provider
     - Environment Variable
     - Default Model
   * - 1
     - Google Vertex AI
     - ``VERTEX_PROJECT_ID``
     - ``vertex::gemini-2.5-flash``
   * - 2
     - Google Gemini
     - ``GEMINI_API_KEY``
     - ``gemini-3.1-flash-lite-preview``
   * - 3
     - OpenAI
     - ``OPENAI_API_KEY``
     - ``gpt-4o-mini``
   * - 4
     - Anthropic
     - ``ANTHROPIC_API_KEY``
     - ``claude-3-5-sonnet-20241022``
   * - 5
     - Groq
     - ``GROQ_API_KEY``
     - ``llama-3.1-8b-instant``
   * - 6
     - Cohere
     - ``COHERE_API_KEY``
     - ``command-r-plus``
   * - 7
     - XAI
     - ``XAI_API_KEY``
     - ``grok-beta``
   * - 8
     - DeepSeek
     - ``DEEPSEEK_API_KEY``
     - ``deepseek-chat``
   * - 9
     - Ollama
     - *(none - auto-detected)*
     - ``llama3.2``

.. code-block:: bash

   # Example: set a key and run
   export GEMINI_API_KEY="your-key"
   perspt                # auto-detects Gemini, uses gemini-3.1-flash-lite-preview
   perspt chat --model gemini-3.1-pro   # override model

Advanced Enterprise Provider Configurations
-------------------------------------------

Unlike standard API-key based providers, enterprise platforms like **Google Agent Platform (formerly Vertex AI)** require multi-part configurations and secure credentials to function.

Google Agent Platform (formerly Vertex AI)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

Google Agent Platform/Vertex AI uses secure OAuth2 Bearer Tokens rather than standard static API keys. You must supply your Google Cloud Project ID and regional location alongside the access token.

**Required Environment Variables:**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Variable
     - Description
   * - ``VERTEX_API_KEY``
     - Your OAuth2 Bearer access token (generate dynamically via gcloud CLI).
   * - ``VERTEX_PROJECT_ID``
     - Your Google Cloud Platform (GCP) Project ID.
   * - ``VERTEX_LOCATION``
     - The GCP region hosting Vertex AI resources (e.g., ``us-central1`` or ``europe-west3``).

**Token Generation Quickstart:**

Since OAuth2 access tokens are short-lived (usually expiring in 1 hour), you can export the token dynamically in your shell before running Perspt:

.. code-block:: bash

   # 1. Authenticate with Google Cloud CLI
   gcloud auth login

   # 2. Configure variables and inject your access token
   export VERTEX_PROJECT_ID="your-gcp-project-123"
   export VERTEX_LOCATION="us-central1"
   export VERTEX_API_KEY=$(gcloud auth print-access-token)

   # 3. Launch Perspt using a Vertex AI model
   perspt chat --model gemini-3.5-flash

Supported Models & Naming Conventions
-------------------------------------

In version 0.6.6, a fully qualified model name resolves its own provider: a
``provider::model`` prefix (``openai``, ``anthropic``, ``gemini``/``google``,
``vertex``, ``groq``, ``cohere``, ``ollama``, ``xai``, or ``deepseek``) selects
that provider directly, and the model part is passed through verbatim. A bare
model name uses the configured or auto-detected provider instead.

.. code-block:: bash

   perspt chat --model vertex::gemini-3.1-pro   # provider from the namespace
   perspt chat --model gemini-3.1-pro           # provider from config/detection


Configuration File
------------------

Perspt reads ``config.toml`` from the platform config directory, or from an
explicit path:

1. Path given via ``perspt --config <PATH>``
2. ``~/.config/perspt/config.toml`` (Linux)
3. ``~/Library/Application Support/perspt/config.toml`` (macOS)
4. ``%APPDATA%\perspt\config.toml`` (Windows)

All fields are optional. ``provider`` accepts the aliases ``provider_type`` and
``default_provider``; ``model`` accepts the alias ``default_model``.

**Minimal example:**

.. code-block:: toml

   provider = "gemini"
   model = "gemini-3.1-pro"
   api_key = "your-key"

**Full example:**

.. code-block:: toml

   provider = "openai"
   model = "phi-4-npu-ov"
   api_key = "your-key"
   # Override the endpoint for OpenAI-compatible / local / proxy servers
   base_url = "http://localhost:8000/v1"

   # Optional per-role routes for `perspt agent`, as fully qualified
   # `provider::model` values. When present, the [models] table takes
   # precedence over the flat *_model fields.
   [models]
   architect = "openai::gpt-5.5"
   actuator = "openai::gpt-5-mini"
   verifier = "openai::gpt-5-mini"
   speculator = "openai::gpt-5-mini"
   adjudicator = "openai::gpt-5.5"

.. note::
   ``base_url`` overrides the endpoint for the active provider. This is useful
   for Azure OpenAI, proxy servers, local OpenAI-compatible servers, or
   self-hosted endpoints. You can also set the provider's ``*_BASE_URL``
   environment variable (``OPENAI_BASE_URL``, ``OLLAMA_BASE_URL``, ...).

.. note::
   Custom model names that genai does not recognize (for example
   ``phi-4-npu-ov``) are routed to the configured ``provider``. You can also
   target an adapter inline with namespacing, e.g. ``openai::phi-4-npu-ov``.

Agent Configuration Blocks
--------------------------

The agent runtime reads four optional TOML blocks. All fields are optional;
invalid values fail at startup.

**Bounded search** (``[exploration]``):

.. code-block:: toml

   [exploration]
   initial_branches = 1        # Branches opened before any expansion trigger
   max_branches = 3            # Branch identities per forest (hard cap 3)
   distinct_family = true      # Prefer a distinct model family on expansion
   max_workspace_files = 2048  # Cumulative eager-copy file reservation cap
   max_workspace_bytes = 134217728  # Cumulative eager-copy byte reservation cap

**Prompt bundles** (``[prompts]``):

.. code-block:: toml

   [prompts]
   bundles = ["./prompt-bundles/tuned"]  # External bundles, pinned at session start
   activation_min_tasks = 30    # Minimum paired activation tasks (floor 30; raise-only)
   noninferiority_margin = 0.05 # Noninferiority margin epsilon in [0, 0.05]

**Resident-context reserves** (``[context]``):

.. code-block:: toml

   [context]
   working_set_turns = 8          # Verbatim turns kept in the working set
   synopsis_frame_tokens = 2048   # Token reserve for the synopsis frame
   output_reserve_tokens = 8192   # Token reserve for model output
   guard_reserve_tokens = 1024    # Guard reserve against overflow

**Verification acceptance and test evidence** (``[verification]``):

.. code-block:: toml

   [verification]
   test_policy = "evolving"  # Default: resulting code, tests, and configuration
   require_format = false    # Declare the plugin format stage as an acceptance sensor
   stage_timeout_secs = 180  # Wall-clock limit for every governed verifier stage
   test_timeout_secs = 300   # Per-stage override (also syntax/build/lint/format)

The test policy defines which test evidence must pass in addition to the
coding domain's required syntax and build stages:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Policy
     - Acceptance behavior
   * - ``evolving``
     - The default for iterative development. Perspt runs the resulting
       implementation, resulting project tests, and resulting configuration.
       Existing tests may be corrected, replaced, or removed when the task
       intentionally changes their contract; newly written tests participate
       in the same gate.
   * - ``backward-compatible``
     - Runs the resulting suite and a second regression view in which
       recognized pre-existing test files are restored. Select it only when
       the task promises compatibility with those historical expectations.
   * - ``external-oracle``
     - Runs the resulting suite and then overlays separately protected
       acceptance material onto a private candidate copy. Its configured
       command is an additional required test-stage verdict. This is intended
       for CI, security fixes, contractual acceptance, and other work with an
       independently maintained suite.

An external oracle is explicit and fail-closed:

.. code-block:: toml

   [verification]
   test_policy = "external-oracle"
   test_timeout_secs = 600

   [verification.external_oracle]
   path = "/srv/project-acceptance"       # or relative to the agent workspace
   command = "cargo test --test acceptance"

The directory is copied over a private copy of the candidate only at the
measurement boundary. It can contain tests, manifests, runner configuration,
or harness scripts, and none of those files are promoted. Keep the directory
outside the workspace when its contents must be withheld from the actuator.
Configuring the table without ``test_policy = "external-oracle"``, or selecting
that policy without the table, is a startup error rather than silently ignored
configuration.

``evolving`` does not claim that model-authored tests are independent proof of
semantic correctness. It means the configured project verification suite
passed for the new contract. Use protected acceptance evidence when an
independent semantic oracle is required.

.. note::
   The ``[ensemble]`` section was removed by PSP-10 and is now a hard startup
   error: the proposal ensemble is replaced by the bounded search forest, and
   the error message points to ``[exploration]``.

Command-Line Flags
------------------

Global flags apply to all subcommands:

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Flag
     - Description
   * - ``-v, --verbose``
     - Enable verbose logging
   * - ``-c, --config <PATH>``
     - Path to configuration file
   * - ``-h, --help``
     - Show help
   * - ``-V, --version``
     - Show version

Chat-specific:

.. code-block:: bash

   perspt chat --model <MODEL>

Agent-specific (see :doc:`howto/agent-options` for the full list):

.. code-block:: bash

   perspt agent [OPTIONS] "<TASK>"

   # Key options:
   --model <MODEL>              # Primary actuator alias
   --actuator-model <MODEL>     # Governed tool-call route
   --explorer-model <MODEL>     # Optional cheap no-tool exploration
   --adjudicator-model <MODEL>  # Optional no-tool diff veto
   --fallback-model <MODEL>     # Repeatable sticky actuator fallback
   -w, --workdir <DIR>          # Working directory
   -y, --yes                    # Auto-approve (headless)
   --rho-gate <V>               # Required measured descent
   --max-turns <N>              # Finite model-turn budget
   --max-calls-per-turn <N>     # Direct and nested call budget
   --rejection-budget <N>       # Shared recovery/rejection budget
   --max-parallel <N>           # Parallel verifier sensors
   --max-parallel-nodes <N>     # Concurrent work-graph nodes (>1 needs --yes)
   --exploration-only           # Read-only exploration; nothing mutated
   --allow-experimental-prompts # Substitute validated [prompts] bundles live
   --domain <ID>                # Domain package (coding, research); default: detect
   --allow-dependency-mutation  # Grant governed dependency mutation
   --persistent-grants          # Sign durable grant intent
   --db-path <PATH>             # PSP-9 ledger database path
   --dashboard                  # Start the web dashboard alongside the agent
   --dashboard-port <N>         # Embedded dashboard port (default 3000)
   --output-summary <FILE>      # Terminal session summary as JSON

Manage configuration interactively:

.. code-block:: bash

   perspt config --show    # Print the effective config (api_key masked)
   perspt config --edit    # Open in $EDITOR
   perspt config --set provider=gemini
   perspt config --set default_model=gemini-3.1-pro

Initialize project-level configuration:

.. code-block:: bash

   perspt init --memory --rules

Dashboard Configuration
-----------------------

The ``perspt dashboard`` subcommand accepts these options:

.. list-table::
   :header-rows: 1
   :widths: 20 20 60

   * - Flag
     - Default
     - Description
   * - ``--port``
     - ``3000``
     - HTTP port for the dashboard server
   * - ``--db-path``
     - Platform default
     - Path to the DuckDB database file

The dashboard opens the database in **read-only** mode and never writes to it.
The server always binds to ``127.0.0.1``; cookies are set without the
``Secure`` flag so plain HTTP works on localhost.
