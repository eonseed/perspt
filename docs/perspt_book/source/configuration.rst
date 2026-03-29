Configuration Guide
===================

Perspt supports zero-config auto-detection, environment variables, a JSON config
file, and command-line flags. They are applied in this priority order (highest first):

1. **Command-line arguments**
2. **Configuration file** (``config.json``)
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
     - ``gpt-4o-mini``
   * - 2
     - Anthropic
     - ``ANTHROPIC_API_KEY``
     - ``claude-sonnet-4-20250514``
   * - 3
     - Google Gemini
     - ``GEMINI_API_KEY``
     - ``gemini-3.1-flash-lite-preview``
   * - 4
     - Groq
     - ``GROQ_API_KEY``
     - ``llama-3.1-70b-versatile``
   * - 5
     - Cohere
     - ``COHERE_API_KEY``
     - ``command-r-plus``
   * - 6
     - XAI
     - ``XAI_API_KEY``
     - ``grok-beta``
   * - 7
     - DeepSeek
     - ``DEEPSEEK_API_KEY``
     - ``deepseek-chat``
   * - 8
     - Ollama
     - *(none — auto-detected)*
     - ``llama3.2``

.. code-block:: bash

   # Example: set a key and run
   export GEMINI_API_KEY="your-key"
   perspt                # auto-detects Gemini, uses gemini-3.1-flash-lite-preview
   perspt chat --model gemini-pro-latest   # override model

Configuration File
------------------

Create ``config.json`` in one of these locations (searched in order):

1. Path given via ``perspt --config <PATH>``
2. ``./config.json`` in current directory
3. ``~/.config/perspt/config.json`` (Linux), ``~/Library/Application Support/perspt/config.json`` (macOS)

**Minimal example:**

.. code-block:: json

   {
     "api_key": "your-key",
     "default_model": "gemini-pro-latest",
     "default_provider": "gemini",
     "provider_type": "gemini"
   }

**Full example:**

.. code-block:: json

   {
     "api_key": "your-key",
     "default_model": "gemini-pro-latest",
     "default_provider": "gemini",
     "provider_type": "gemini",
     "providers": {
       "openai": "https://api.openai.com/v1",
       "anthropic": "https://api.anthropic.com",
       "google": "https://generativelanguage.googleapis.com/v1beta"
     }
   }

.. note::
   The ``providers`` map overrides endpoint URLs. This is useful for Azure OpenAI,
   proxy servers, or self-hosted endpoints.

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
   --energy-weights <a,b,g>     # Custom alpha,beta,gamma
   --stability-threshold <e>    # Custom epsilon
   --log-llm                    # Log all LLM calls to DB
   --single-file                # Force single-file mode
   --verifier-strictness <LVL>  # default | strict | minimal

Manage configuration interactively:

.. code-block:: bash

   perspt config --show    # Print current config
   perspt config --edit    # Open in $EDITOR
   perspt config --set provider_type=gemini

Initialize project-level configuration:

.. code-block:: bash

   perspt init --memory --rules
