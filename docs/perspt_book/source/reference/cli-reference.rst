.. _cli-reference:

CLI Reference
=============

Complete command-line options for Perspt.

Global Options
--------------

.. code-block:: text

   perspt [OPTIONS]

.. list-table::
   :widths: 35 65
   :header-rows: 1

   * - Option
     - Description
   * - ``-c, --config <FILE>``
     - Configuration file path
   * - ``-k, --api-key <KEY>``
     - API key (overrides config/env)
   * - ``-m, --model <MODEL>``
     - Model name
   * - ``-p, --provider-type <TYPE>``
     - Provider: openai, anthropic, gemini, groq, cohere, xai, deepseek, ollama
   * - ``--provider <PROFILE>``
     - Provider profile from config
   * - ``-l, --list-models``
     - List available models
   * - ``--simple-cli``
     - Minimal CLI mode (no TUI)
   * - ``--log-file <FILE>``
     - Log session (simple-cli only)
   * - ``-h, --help``
     - Show help
   * - ``-V, --version``
     - Show version

Chat Commands
-------------

In-session commands (TUI mode):

.. list-table::
   :widths: 30 70
   :header-rows: 1

   * - Command
     - Description
   * - ``/save``
     - Save with timestamp
   * - ``/save <file>``
     - Save to specific file

Keyboard Shortcuts
------------------

.. list-table::
   :widths: 20 80
   :header-rows: 1

   * - Key
     - Action
   * - **Enter**
     - Send message
   * - **Ctrl+C**
     - Exit
   * - **Ctrl+D**
     - Exit (simple-cli)
   * - **↑/↓**
     - Scroll history
   * - **Page Up/Down**
     - Fast scroll

Agent Mode
----------

.. code-block:: text

   perspt agent [OPTIONS] <TASK>

See :doc:`/howto/agent-options` for details.

Environment Variables
---------------------

.. list-table::
   :widths: 35 65
   :header-rows: 1

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
     - XAI API key
   * - ``DEEPSEEK_API_KEY``
     - DeepSeek API key
