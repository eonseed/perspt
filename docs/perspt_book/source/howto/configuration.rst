.. _howto-configuration:

Configuration Guide
===================

Set up API keys, models, and preferences.

Environment Variables
---------------------

The simplest configuration method:

.. code-block:: bash

   # Set your API key
   export OPENAI_API_KEY="sk-..."
   
   # Perspt auto-detects and uses it
   perspt

**Detection priority**: OpenAI → Anthropic → Google → Groq → Cohere → XAI → DeepSeek → Ollama

Configuration File
------------------

Create ``config.json`` for persistent settings:

.. code-block:: json

   {
     "provider_type": "openai",
     "default_model": "gpt-4o-mini",
     "api_key": "sk-..."
   }

Use it:

.. code-block:: bash

   perspt --config config.json

CLI Overrides
-------------

Command-line arguments override config files:

.. code-block:: bash

   perspt --provider-type anthropic --model claude-3-5-sonnet-20241022

See :doc:`/reference/cli-reference` for all options.

Agent Mode Configuration
------------------------

For agent mode, additional options control behavior:

.. code-block:: bash

   perspt agent \
     --workspace ./project \
     --max-tokens 50000 \
     --max-cost 1.00 \
     "Create a REST API"

See :doc:`agent-options` for details.
