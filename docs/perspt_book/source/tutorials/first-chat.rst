.. _tutorial-first-chat:

First Chat
==========

Start your first conversation with an LLM using Perspt.

Prerequisites
-------------

- Perspt installed (see :doc:`../installation`)
- An API key from any provider, or Ollama running locally

Step 1: Set Your API Key
-------------------------

.. code-block:: bash

   export GEMINI_API_KEY="your-key"

Step 2: Launch the TUI
-----------------------

.. code-block:: bash

   perspt

Perspt auto-detects the provider from the environment variable and launches the
chat TUI. You will see a status bar showing the provider and model.

Step 3: Chat
------------

Type a message and press **Enter**. Perspt streams the response in real time with
markdown formatting (code blocks, headers, lists, bold, italic).

.. code-block:: text

   You: Explain Rust's ownership model in 3 sentences.

   Assistant: Rust's ownership model ensures each value has exactly one owner at a
   time. When the owner goes out of scope, the value is automatically dropped.
   Borrowing rules allow temporary references without transferring ownership, and
   the compiler enforces these rules at compile time to prevent data races and
   dangling pointers.

Step 4: Navigate
----------------

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
     - Scroll one line
   * - **Esc** or **Ctrl+Q**
     - Exit
   * - **/save**
     - Save conversation to file

Step 5: Try Simple CLI Mode
----------------------------

For a minimal interface without the TUI:

.. code-block:: bash

   perspt simple-chat
   # Or with logging:
   perspt simple-chat --log-file session.txt

Type your question, get a streamed text answer. Type ``exit`` or ``Ctrl+D`` to quit.

Next Steps
----------

- :doc:`agent-mode` — Try autonomous multi-file coding
- :doc:`local-models` — Use Ollama for offline conversations
