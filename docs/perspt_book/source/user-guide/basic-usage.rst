.. _user-guide-basic-usage:

Basic Usage
===========

Perspt offers two interactive modes: the **TUI** (rich terminal UI) and
**simple-chat** (plain-text streaming).

Launching the TUI
------------------

.. code-block:: bash

   # Auto-detect provider from env keys
   perspt

   # Explicit provider + model
   perspt chat --provider-type anthropic --model claude-sonnet-4-20250514

   # Override API key
   perspt chat --api-key sk-xxx --model gpt-4.1

The TUI provides:

- Markdown rendering (code blocks, headers, lists, bold, italic)
- Real-time response streaming
- Scroll navigation
- Status bar (provider, model, streaming indicator)

Keyboard Shortcuts
------------------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Key
     - Action
   * - **Enter**
     - Send message
   * - **Ctrl+J**
     - Insert newline in input
   * - **Page Up / Down**
     - Scroll chat history
   * - **Ctrl+Up / Down**
     - Scroll by one line
   * - **Home / End**
     - Jump to top / bottom
   * - **Esc** or **Ctrl+Q**
     - Quit
   * - **Ctrl+C**
     - Cancel current stream

Chat Commands
-------------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Command
     - Description
   * - ``/save``
     - Save conversation to a timestamped text file
   * - ``exit`` or ``quit``
     - Exit the application

Simple CLI Mode
---------------

For a minimal text interface suitable for piping and logging:

.. code-block:: bash

   perspt simple-chat
   perspt simple-chat --log-file session.txt

Type your message, press Enter. Responses stream to stdout. Type ``exit`` or
press ``Ctrl+D`` to quit.


Provider Auto-Detection
-----------------------

Perspt checks environment variables in this priority order:

.. list-table::
   :header-rows: 1
   :widths: 30 30 40

   * - Env Variable
     - Provider
     - Default Model
   * - ``ANTHROPIC_API_KEY``
     - Anthropic
     - ``claude-sonnet-4-20250514``
   * - ``OPENAI_API_KEY``
     - OpenAI
     - ``gpt-4.1``
   * - ``GEMINI_API_KEY``
     - Gemini
     - ``gemini-3.1-flash-lite-preview``
   * - ``GROQ_API_KEY``
     - Groq
     - ``llama-3.3-70b-versatile``
   * - ``COHERE_API_KEY``
     - Cohere
     - ``command-r-plus``
   * - ``XAI_API_KEY``
     - xAI
     - ``grok-3-mini-fast``
   * - ``DEEPSEEK_API_KEY``
     - DeepSeek
     - ``deepseek-chat``
   * - *(none)*
     - Ollama (local)
     - ``llama3.2``

See :doc:`providers` for full provider details.
