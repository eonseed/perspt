.. _tutorial-first-chat:

First Chat
==========

Your first conversation with an LLM using Perspt.

Prerequisites
-------------

- Perspt installed (see :doc:`../quickstart`)
- An API key for any provider

Step 1: Set Your API Key
------------------------

Choose your provider and set the environment variable:

.. tab-set::

   .. tab-item:: OpenAI

      .. code-block:: bash

         export OPENAI_API_KEY="sk-..."

   .. tab-item:: Anthropic

      .. code-block:: bash

         export ANTHROPIC_API_KEY="sk-ant-..."

   .. tab-item:: Google

      .. code-block:: bash

         export GEMINI_API_KEY="..."

   .. tab-item:: Ollama (Local)

      .. code-block:: bash

         # No key needed, just ensure Ollama is running
         ollama serve

Step 2: Launch Perspt
---------------------

.. code-block:: bash

   perspt

Or with a specific model:

.. code-block:: bash

   perspt chat --model gpt-5.2

Step 3: The TUI Interface
-------------------------

You'll see the Perspt TUI:

.. code-block:: text

   ┌─────────────────────────────────────────────────────────────┐
   │  Perspt v0.5.0 - gpt-5.2                     Tokens: 0     │
   ├─────────────────────────────────────────────────────────────┤
   │                                                             │
   │                                                             │
   │                   Welcome to Perspt!                        │
   │            Your Terminal's Window to the AI World           │
   │                                                             │
   │                                                             │
   ├─────────────────────────────────────────────────────────────┤
   │  > Type your message here...                                │
   └─────────────────────────────────────────────────────────────┘

Step 4: Send a Message
----------------------

Type your message and press **Enter**:

.. code-block:: text

   > What is the capital of France?

The response will stream in real-time with markdown rendering.

Step 5: Continue the Conversation
---------------------------------

Keep chatting! The conversation history is maintained:

.. code-block:: text

   > And what's the population?

   The population of Paris is approximately 2.1 million in the city
   proper, and about 12 million in the metropolitan area.

Step 6: Save Your Conversation
------------------------------

Use the ``/save`` command:

.. code-block:: text

   > /save my_chat.md

Or with automatic timestamp:

.. code-block:: text

   > /save

Step 7: Exit
------------

Press **Esc** or **Ctrl+C** to exit cleanly.

Key Bindings Reference
----------------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Key
     - Action
   * - **Enter**
     - Send message
   * - **Esc**
     - Exit application
   * - **↑/↓**
     - Scroll chat history
   * - **Page Up/Down**
     - Fast scroll
   * - **Ctrl+C**
     - Force exit

Tips
----

1. **Markdown works**: Use ``code``, **bold**, and lists in your prompts
2. **Long responses**: Scroll up to see earlier content
3. **Token tracking**: Watch the token counter in the header
4. **Model switching**: Use ``perspt chat --model <name>`` for different models

Next Steps
----------

- :doc:`local-models` — Use Ollama for offline AI
- :doc:`agent-mode` — Try autonomous code generation
- :doc:`../howto/configuration` — Customize your setup
