.. _user-guide-basic-usage:

Basic Usage
===========

Perspt offers three primary modes of interaction: the **TUI** (rich interactive terminal UI),
**simple-chat** (plain-text streaming for pipeline integration), and **Agent** (self-stabilizing autonomous multi-agent coding workflows).

Launching the TUI
------------------

.. code-block:: bash

   # Auto-detect provider from env keys
   perspt

   # Explicit model (provider comes from config or env detection)
   perspt chat --model claude-fable

   # Use a specific config file
   perspt --config ./config.toml chat --model gpt-5.5

The TUI provides:

- Markdown rendering (code blocks, headers, lists, bold, italic)
- Premium ASCII table rendering with multi-line cell wrapping
- Inline LaTeX math transpilation to styled Unicode equations (bold and cyan)
- Real-time response streaming
- Scroll navigation
- Status bar (provider, model, streaming indicator, and reasoning toggles)

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
     - Scroll chat history by 10 lines
   * - **Shift+Up / Down**
     - Scroll chat history by 1 line
   * - **Ctrl+Up / Down**
     - Scroll chat history by 1 line
   * - **Home / End**
     - Go to start / end of input line
   * - **Ctrl+C** / **Ctrl+Q**
     - Quit
   * - **Ctrl+R**
     - Toggle inner reasoning process display
   * - **Ctrl+A** / **Ctrl+E**
     - Go to start / end of input line
   * - **Ctrl+B** / **Ctrl+F**
     - Move cursor left / right
   * - **Ctrl+D** / **Ctrl+H**
     - Delete / Backspace character
   * - **Ctrl+K** / **Ctrl+U**
     - Kill to end / start of line
   * - **Ctrl+W**
     - Delete word before cursor

Slash Commands (Directives)
---------------------------

Both CLI and TUI modes support slash commands typed directly in the input box:

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Command
     - Description
   * - ``/exit`` or ``/quit``
     - Exit the application session
   * - ``/clear``
     - Reset the active conversation history
   * - ``/model <name>``
     - Switch the active LLM model on the fly
   * - ``/save <path>`` (TUI only)
     - Export full conversation history to a markdown file
   * - ``/mcp`` (TUI only)
     - Show MCP configuration state, discovery failures, policy rejections,
       and admitted operation names without contacting the model
   * - ``/mcp accept [JSON]``, ``decline``, or ``cancel`` (TUI only)
     - Answer a pending MCP elicitation request
   * - ``/help``
     - Print the menu of available slash commands

The chat TUI can use latest MCP tools, resources, prompts, completions,
sampling, elicitation, subscriptions, and tasks from any ``[[external_tools]]``
server entry in ``config.toml`` with ``modes = ["chat"]`` or
``modes = ["agent", "chat"]`` is considered for read-only admission. Every
allowed remote tool also needs an exact local policy table. Tool activity is
shown as a concise transient status only while a call is active. MCP protocol
events, arguments, results, discovery notices, and subscription traces do not
become conversation or reasoning content. The selected model chooses an
admitted tool automatically when needed; ``/mcp`` is the explicit diagnostic
surface. Tool results are labeled as untrusted content before returning to the
model. **Ctrl+R** continues to control genuine model reasoning, including
tool-aware turns. Unicode and multiline terminal clipboard paste is inserted
at the input cursor. See :doc:`mcp` for configuration and the security
boundary.

Simple CLI Mode
---------------

For a minimal text interface suitable for piping and logging:

.. code-block:: bash

   perspt simple-chat
   perspt simple-chat --model gpt-4o-mini
   perspt simple-chat --log-file session.txt

Type your message, press Enter. Responses stream to stdout. Type ``exit`` or
press ``Ctrl+D`` to quit.

Autonomous Agent Mode
---------------------

For complex software engineering and coding jobs that require self-stabilizing, multi-file edits:

.. code-block:: bash

   perspt agent "Implement a parsing logic for custom CSV formats in src/csv.rs"

This starts the multi-agent orchestration loop. The system automatically plans, executes, and verifies the changes using local verification commands. You can specify custom models and configure validation parameters to match your codebase. Refer to :doc:`agent-mode` for details.


Provider Auto-Detection
-----------------------

Perspt checks environment variables in this priority order:

.. list-table::
   :header-rows: 1
   :widths: 30 30 40

   * - Env Variable
     - Provider
     - Default Model
   * - ``VERTEX_PROJECT_ID``
     - Vertex AI
     - ``vertex::gemini-2.5-flash``
   * - ``GEMINI_API_KEY``
     - Gemini
     - ``gemini-3.1-flash-lite-preview``
   * - ``OPENAI_API_KEY``
     - OpenAI
     - ``gpt-4o-mini``
   * - ``ANTHROPIC_API_KEY``
     - Anthropic
     - ``claude-3-5-sonnet-20241022``
   * - ``GROQ_API_KEY``
     - Groq
     - ``llama-3.1-8b-instant``
   * - ``COHERE_API_KEY``
     - Cohere
     - ``command-r-plus``
   * - ``XAI_API_KEY``
     - xAI
     - ``grok-beta``
   * - ``DEEPSEEK_API_KEY``
     - DeepSeek
     - ``deepseek-chat``
   * - *(none)*
     - Ollama (local)
     - ``llama3.2``

See :doc:`providers` for full provider details.
