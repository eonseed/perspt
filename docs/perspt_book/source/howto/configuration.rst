.. _howto-configuration:

Configuration
=============

How to configure Perspt for your workflow.

Configuration Sources
---------------------

Perspt loads configuration from (highest priority first):

1. **CLI Arguments** — ``perspt --model gpt-5.2``
2. **Environment Variables** — ``export OPENAI_API_KEY=...``
3. **Config File** — ``~/.perspt/config.toml``
4. **Defaults** — Built-in fallbacks

Config File Location
--------------------

.. code-block:: text

   ~/.perspt/config.toml

Create it:

.. code-block:: bash

   mkdir -p ~/.perspt
   perspt config --edit

Config File Format
------------------

.. code-block:: toml

   # ~/.perspt/config.toml

   [default]
   provider = "openai"
   model = "gpt-5.2"

   [providers.openai]
   api_key = "sk-..."
   
   [providers.anthropic]
   api_key = "sk-ant-..."

   [agent]
   architect_model = "gpt-5.2"
   actuator_model = "claude-opus-4.5"
   verifier_model = "gemini-3-pro"
   energy_weights = [1.0, 0.5, 2.0]
   stability_threshold = 0.1
   max_retries_compile = 3
   max_retries_tool = 5

Environment Variables
---------------------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Variable
     - Description
   * - ``OPENAI_API_KEY``
     - OpenAI API key
   * - ``ANTHROPIC_API_KEY``
     - Anthropic API key
   * - ``GEMINI_API_KEY``
     - Google Gemini API key
   * - ``GROQ_API_KEY``
     - Groq API key
   * - ``COHERE_API_KEY``
     - Cohere API key
   * - ``XAI_API_KEY``
     - XAI (Grok) API key
   * - ``DEEPSEEK_API_KEY``
     - DeepSeek API key

CLI Configuration Commands
--------------------------

.. code-block:: bash

   # Show current configuration
   perspt config --show

   # Set a value
   perspt config --set default.model=gpt-5.2

   # Edit in $EDITOR
   perspt config --edit

Project Configuration
---------------------

Initialize project-specific config:

.. code-block:: bash

   cd my-project
   perspt init --memory --rules

This creates:

.. code-block:: text

   my-project/
   ├── PERSPT.md         # Project memory/context
   └── .perspt/
       ├── config.toml   # Project config
       └── rules.star    # Policy rules

PERSPT.md
~~~~~~~~~

Project memory file that provides context to the agent:

.. code-block:: markdown

   # My Project

   ## Overview
   This is a Python web application using FastAPI.

   ## Architecture
   - `api/` - REST endpoints
   - `core/` - Business logic
   - `tests/` - pytest suite

   ## Conventions
   - Use type hints everywhere
   - 100% test coverage required

Per-Session Configuration
-------------------------

Override for a single session:

.. code-block:: bash

   perspt chat --model claude-opus-4.5

   perspt agent \
     --architect-model gpt-5.2 \
     --actuator-model claude-opus-4.5 \
     "Create module"

See Also
--------

- :doc:`providers` - Provider-specific setup
- :doc:`agent-options` - Agent configuration
- :doc:`security-rules` - Policy rules
