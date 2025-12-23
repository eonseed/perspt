.. _first-chat:

First Chat Tutorial
===================

Get chatting with an AI in 5 minutes.

Step 1: Set Your API Key
------------------------

.. tabs::

   .. tab:: OpenAI

      .. code-block:: bash

         export OPENAI_API_KEY="sk-your-key-here"

   .. tab:: Anthropic

      .. code-block:: bash

         export ANTHROPIC_API_KEY="sk-ant-your-key-here"

   .. tab:: Google

      .. code-block:: bash

         export GEMINI_API_KEY="your-key-here"

   .. tab:: Ollama (Local)

      .. code-block:: bash

         # No API key needed - just start Ollama
         ollama serve
         ollama pull llama3.2

Step 2: Launch Perspt
---------------------

.. code-block:: bash

   perspt

You'll see:

.. code-block:: text

   Perspt v0.5.0 - Performance LLM Chat CLI
   Provider: OpenAI | Model: gpt-4o-mini | Status: Connected ✓

   >

Step 3: Start Chatting
----------------------

Type a message and press Enter:

.. code-block:: text

   > What is Rust?

   Rust is a systems programming language focused on safety,
   concurrency, and performance...

   > Can you show me a simple example?

   Here's a simple "Hello, World!" in Rust:

   ```rust
   fn main() {
       println!("Hello, World!");
   }
   ```

Step 4: Save Your Conversation
------------------------------

Use the ``/save`` command:

.. code-block:: text

   > /save
   💾 Conversation saved to: conversation_1735123456.txt

   > /save my-notes.txt
   💾 Conversation saved to: my-notes.txt

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
   * - **↑/↓**
     - Scroll history
   * - **Page Up/Down**
     - Fast scroll

Try Simple CLI Mode
-------------------

For scripting or accessibility:

.. code-block:: bash

   perspt --simple-cli

   # With session logging
   perspt --simple-cli --log-file session.txt

Next Steps
----------

- :doc:`agent-mode` - Autonomous code generation
- :doc:`local-models` - Set up Ollama
- :doc:`/howto/providers` - Configure other providers
