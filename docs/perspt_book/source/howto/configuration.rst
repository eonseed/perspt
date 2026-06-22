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

   # Optional per-tier overrides for `perspt agent`
   # architect_model = "gpt-5.5"
   # actuator_model = "gpt-5-mini"
   # verifier_model = "gpt-5-mini"
   # speculator_model = "gpt-5-mini"

Environment Variables
---------------------

.. list-table::
   :header-rows: 1
   :widths: 35 20 45

   * - Variable
     - Provider
     - Priority
   * - ``ANTHROPIC_API_KEY``
     - Anthropic
     - Highest
   * - ``OPENAI_API_KEY``
     - OpenAI
     - 2
   * - ``GEMINI_API_KEY``
     - Gemini
     - 3
   * - ``GROQ_API_KEY``
     - Groq
     - 4
   * - ``COHERE_API_KEY``
     - Cohere
     - 5
   * - ``XAI_API_KEY``
     - xAI
     - 6
   * - ``DEEPSEEK_API_KEY``
     - DeepSeek
     - 7
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

   # Agent LLM logging to DuckDB
   perspt agent --log-llm -w . "Task"
