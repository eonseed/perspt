.. _user-guide-basic-usage:

Basic Usage
===========

Day-to-day operations with Perspt.

Starting Perspt
---------------

.. code-block:: bash

   # Default: starts chat TUI
   perspt

   # With specific model
   perspt chat --model gpt-5.2

   # From anywhere
   export PATH="$PATH:/path/to/perspt/target/release"
   perspt

Chat Interface
--------------

.. code-block:: text

   ┌─────────────────────────────────────────────────────────────┐
   │  Perspt v0.5.0 - gpt-5.2                     Tokens: 1,234 │
   ├─────────────────────────────────────────────────────────────┤
   │                                                             │
   │  User: What is recursion?                                   │
   │                                                             │
   │  Assistant: Recursion is a programming technique where a    │
   │  function calls itself to solve a problem by breaking it    │
   │  down into smaller subproblems.                            │
   │                                                             │
   │  ```python                                                  │
   │  def factorial(n):                                          │
   │      if n <= 1:                                             │
   │          return 1                                           │
   │      return n * factorial(n - 1)                            │
   │  ```                                                        │
   │                                                             │
   ├─────────────────────────────────────────────────────────────┤
   │  > _                                                        │
   └─────────────────────────────────────────────────────────────┘

Sending Messages
----------------

1. Type your message in the input area
2. Press **Enter** to send
3. Watch the streamed response with markdown rendering

Key Actions
-----------

.. list-table::
   :widths: 30 70

   * - **Enter**
     - Send message
   * - **Ctrl+J** or **Ctrl+Enter**
     - Insert newline in input
   * - **PageUp / PageDown**
     - Scroll message history (10 lines)
   * - **Ctrl+Up / Ctrl+Down**
     - Scroll message history (1 line)
   * - **Mouse Scroll**
     - Scroll message history (3 lines)
   * - **Ctrl+C** or **Ctrl+Q**
     - Exit Perspt

Scrolling Behavior
------------------

The chat interface features intelligent auto-scroll:

- **During streaming**: Automatically scrolls to show new content
- **Manual scroll up**: Disables auto-scroll so you can read previous messages
- **Scroll to bottom**: Re-enables auto-scroll when you reach the end

The TUI uses virtual scrolling to handle very long conversations efficiently,
rendering only the visible portion of the message history.

Saving Conversations
--------------------

.. code-block:: text

   > /save
   Saved to: conversation_2024-12-23_15-30-00.md

   > /save my_chat.md
   Saved to: my_chat.md

Switching Models
----------------

.. code-block:: bash

   # In a new session
   perspt chat --model claude-opus-4.5

   # In chat (if supported)
   > /model gemini-3-flash

Working with Code
-----------------

Perspt renders code blocks with syntax highlighting:

.. code-block:: text

   > Write a Python function to reverse a string

   Here's a concise solution:

   ```python
   def reverse_string(s: str) -> str:
       return s[::-1]

   # Usage
   print(reverse_string("hello"))  # "olleh"
   ```

Multi-turn Conversations
------------------------

The chat maintains context:

.. code-block:: text

   > Write a Calculator class

   ```python
   class Calculator:
       def add(self, a, b): return a + b
       def subtract(self, a, b): return a - b
   ```

   > Add multiply and divide methods

   ```python
   class Calculator:
       def add(self, a, b): return a + b
       def subtract(self, a, b): return a - b
       def multiply(self, a, b): return a * b
       def divide(self, a, b): 
           if b == 0: raise ValueError("Cannot divide by zero")
           return a / b
   ```

Token Usage
-----------

The header shows cumulative token usage:

- **Input tokens**: Your prompts
- **Output tokens**: AI responses
- **Total**: Running sum for cost estimation

Exit
----

Press **Esc** or **Ctrl+C** for a clean exit.

See Also
--------

- :doc:`advanced-features` - Power user features
- :doc:`agent-mode` - Autonomous coding
- :doc:`../howto/configuration` - Configuration
