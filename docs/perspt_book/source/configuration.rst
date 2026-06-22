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
     - OpenAI
     - ``OPENAI_API_KEY``
     - ``gpt-5-mini``
   * - 2
     - Anthropic
     - ``ANTHROPIC_API_KEY``
     - ``claude-fable``
   * - 3
     - Google Gemini
     - ``GEMINI_API_KEY``
     - ``gemini-3.5-flash``
   * - 4
     - Groq
     - ``GROQ_API_KEY``
     - ``llama-4-70b``
   * - 5
     - Cohere
     - ``COHERE_API_KEY``
     - ``command-r7``
   * - 6
     - XAI
     - ``XAI_API_KEY``
     - ``grok-4``
   * - 7
     - DeepSeek
     - ``DEEPSEEK_API_KEY``
     - ``deepseek-v4``
   * - 8
     - AWS Bedrock
     - ``AWS_ACCESS_KEY_ID`` (and region/creds)
     - ``us.amazon.nova-pro-v2:0``
   * - 9
     - Google Agent Platform
     - ``VERTEX_API_KEY`` (and project/region)
     - ``vertex::gemini-3.5-flash``
   * - 10
     - Ollama
     - *(none - auto-detected)*
     - ``llama4``

.. code-block:: bash

   # Example: set a key and run
   export GEMINI_API_KEY="your-key"
   perspt                # auto-detects Gemini, uses gemini-3.5-flash
   perspt chat --model gemini-3.1-pro   # override model

Advanced Enterprise Provider Configurations
-------------------------------------------

Unlike standard API-key based providers, enterprise platforms like **AWS Bedrock** and **Google Agent Platform (formerly Vertex AI)** require multi-part configurations and secure credentials to function.

AWS Bedrock (SigV4 Authentication)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

Perspt integrates natively with AWS Bedrock via the AWS Signature Version 4 (SigV4) protocol. It automatically detects your AWS configuration from standard AWS environment variables or your local AWS config files.

**Required Environment Variables:**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Variable
     - Description
   * - ``AWS_ACCESS_KEY_ID``
     - Your AWS Access Key ID.
   * - ``AWS_SECRET_ACCESS_KEY``
     - Your AWS Secret Access Key.
   * - ``AWS_REGION``
     - The AWS region hosting Bedrock (e.g., ``us-east-1`` or ``us-west-2``).
   * - ``AWS_SESSION_TOKEN``
     - *(Optional)* Required if using temporary AWS IAM credentials.

**Local Profile Configuration:**

If environment variables are not set, Perspt will automatically read credentials from your local profile (e.g., ``~/.aws/credentials`` and ``~/.aws/config``) using the standard AWS resolution chain:

.. code-block:: ini

   # ~/.aws/config
   [default]
   region = us-east-1

   # ~/.aws/credentials
   [default]
   aws_access_key_id = AKIAIOSFODNN7EXAMPLE
   aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY

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

Perspt features an intelligent router that resolves your target provider from the model name prefix. In version 0.6.1, we support the latest generation of models across all providers:

.. list-table::
   :header-rows: 1
   :widths: 25 35 40

   * - Provider
     - Model Prefix / Name
     - Target Model ID
   * - **OpenAI**
     - ``gpt-5.5-*`` / ``gpt-5-*``
     - ``gpt-5.5-preview``, ``gpt-5-mini``
   * - **Anthropic**
     - ``claude-fable`` / ``claude-4.8-*``
     - ``claude-fable``, ``opus-4.8``
   * - **Google Gemini**
     - ``gemini-3.5-*`` / ``gemini-3.1-*``
     - ``gemini-3.5-flash``, ``gemini-3.1-pro``
   * - **AWS Bedrock**
     - ``aws.*`` / ``bedrock.*``
     - ``us.amazon.nova-pro-v2:0``, ``us.anthropic.claude-fable-v1:0``
   * - **Google Agent Platform**
     - ``vertex.*``
     - ``vertex::gemini-3.5-flash``, ``vertex::gemini-3.1-pro``


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

   # Optional per-tier overrides for `perspt agent`
   architect_model = "gpt-5.5"
   actuator_model = "gpt-5-mini"
   verifier_model = "gpt-5-mini"
   speculator_model = "gpt-5-mini"

.. note::
   ``base_url`` overrides the endpoint for the active provider. This is useful
   for Azure OpenAI, proxy servers, local OpenAI-compatible servers, or
   self-hosted endpoints. You can also set the provider's ``*_BASE_URL``
   environment variable (``OPENAI_BASE_URL``, ``OLLAMA_BASE_URL``, ...).

.. note::
   Custom model names that genai does not recognize (for example
   ``phi-4-npu-ov``) are routed to the configured ``provider``. You can also
   target an adapter inline with namespacing, e.g. ``openai::phi-4-npu-ov``.

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
   --model <MODEL>              # Default model for all tiers
   --architect-model <MODEL>    # Architect tier
   --actuator-model <MODEL>     # Actuator tier
   --verifier-model <MODEL>     # Verifier tier
   --speculator-model <MODEL>   # Speculator tier
   -w, --workdir <DIR>          # Working directory
   -y, --yes                    # Auto-approve (headless)
   --defer-tests                # Skip V_log during coding
   --mode <MODE>                # cautious | balanced | yolo
   --max-cost <USD>             # Maximum cost in USD
   --max-steps <N>              # Maximum iterations
   --energy-weights <a,b,g>     # Proportional syn/str/log component scales (default 1.0,0.5,2.0)
   --stability-threshold <e>    # Custom epsilon
   --log-llm                    # Log all LLM calls to DB
   --single-file                # Force single-file mode
   --verifier-strictness <LVL>  # default | strict | minimal

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
   * - ``--bind``
     - ``127.0.0.1``
     - Bind address (use ``0.0.0.0`` for remote access)
   * - ``--db-path``
     - Platform default
     - Path to the DuckDB database file

The dashboard opens the database in **read-only** mode and never writes to it.
When bound to ``127.0.0.1``, cookies are set without the ``Secure`` flag so
plain HTTP works on localhost.
