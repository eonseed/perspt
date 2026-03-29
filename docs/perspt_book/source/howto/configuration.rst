.. _howto-configuration:

Configuration
=============

Config File Location
--------------------

Perspt searches for configuration in this order:

1. ``--config <path>`` (CLI flag)
2. ``~/.config/perspt/config.json``
3. Environment variables
4. Auto-detection

Config File Format
------------------

.. code-block:: json

   {
     "default_provider": "gemini",
     "default_model": "gemini-3.1-flash-lite-preview",
     "api_key": "AIza...",
     "provider_type": "gemini",
     "providers": {
       "openai": {
         "api_key": "sk-xxx",
         "default_model": "gpt-4.1"
       },
       "anthropic": {
         "api_key": "sk-ant-xxx",
         "default_model": "claude-sonnet-4-20250514"
       }
     }
   }

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
   with ``--provider-type``.


CLI Flag Override
-----------------

CLI flags always take precedence:

.. code-block:: bash

   # Override provider
   perspt chat --provider-type openai --model gpt-4.1

   # Override API key
   perspt chat --api-key "sk-xxx"

   # Use a specific config file
   perspt chat --config /path/to/config.json

Logging Configuration
---------------------

.. code-block:: bash

   # Default: error-level logging only (avoids TUI noise)
   perspt

   # Enable debug logging with RUST_LOG
   RUST_LOG=debug perspt simple-chat

   # Agent LLM logging to DuckDB
   perspt agent --log-llm -w . "Task"
