Basic Usage
===========

This guide covers the fundamental usage patterns of Perspt, from starting your first conversation to understanding the basic commands and features.

Starting Perspt
----------------

Once Perspt is installed and configured, you can start it with a simple command:

.. code-block:: bash

   perspt

This will launch Perspt with your default configuration. You can also specify a custom configuration file:

.. code-block:: bash

   perspt --config /path/to/your/config.json

Your First Conversation
------------------------

When Perspt starts, you'll see a clean interface ready for interaction:

.. code-block:: text

   Perspt - Personal Perspective Tool
   Type your message and press Enter to start a conversation.
   Type 'help' for available commands.
   
   > 

Simply type your message or question and press Enter:

.. code-block:: text

   > Hello, can you help me understand quantum computing?

The AI will respond based on your configured provider and model. The conversation flows naturally, with context maintained throughout the session.

Basic Commands
--------------

Perspt includes several built-in commands to enhance your experience:

Help Command
~~~~~~~~~~~~

Get information about available commands:

.. code-block:: text

   > /help

This displays all available commands and their descriptions.

Clear Screen
~~~~~~~~~~~~

Clear the conversation history from the display:

.. code-block:: text

   > /clear

.. note::
   This only clears the display. The conversation context is still maintained for the AI.

Exit
~~~~

Exit Perspt gracefully:

.. code-block:: text

   > /exit

Or use the keyboard shortcut ``Ctrl+C``.

Status
~~~~~~

Check the current configuration and connection status:

.. code-block:: text

   > /status

This shows:

- Current AI provider and model
- Connection status
- Configuration file location
- Available commands

Model Switching
~~~~~~~~~~~~~~~

If you have multiple models configured, you can switch between them:

.. code-block:: text

   > /model gpt-4
   > /model claude-3-opus

.. note::
   Model availability depends on your configuration and provider setup.

Managing Conversations
----------------------

Context Awareness
~~~~~~~~~~~~~~~~~

Perspt maintains conversation context throughout your session. The AI remembers:

- Previous messages in the conversation
- Established context and preferences
- Ongoing topics and threads

Example of context-aware conversation:

.. code-block:: text

   > I'm working on a Python project
   AI: I'd be happy to help with your Python project! What specific aspect are you working on?
   
   > It involves web scraping
   AI: Great! For web scraping in Python, you have several excellent options...
   
   > Which library would you recommend for JavaScript-heavy sites?
   AI: For JavaScript-heavy sites that you mentioned for your Python web scraping project, 
        I'd recommend Selenium or Playwright...

Conversation Flow
~~~~~~~~~~~~~~~~~

- **Natural Language**: Write naturally as you would to a human assistant
- **Follow-up Questions**: Ask clarifying questions without repeating context
- **Topic Changes**: Smoothly transition between topics within the same session
- **Code Discussions**: Share code snippets and get detailed feedback

Message Formatting
------------------

Perspt supports rich text formatting in conversations:

Code Blocks
~~~~~~~~~~~

Share code by using triple backticks:

.. code-block:: text

   > Here's my Python function:
   ```python
   def fibonacci(n):
       if n <= 1:
           return n
       return fibonacci(n-1) + fibonacci(n-2)
   ```
   
   Can you help me optimize this?

Long Messages
~~~~~~~~~~~~~

For long messages, you can use multiple lines. Press ``Shift+Enter`` for line breaks:

.. code-block:: text

   > I have a complex question about my architecture:
   
   I'm building a microservices system with the following components:
   - User service (handles authentication)
   - Product service (manages catalog)
   - Order service (processes purchases)
   
   How should I handle cross-service communication?

File Discussions
~~~~~~~~~~~~~~~~

Reference files in your project:

.. code-block:: text

   > I'm having trouble with my config.js file. The authentication isn't working properly.

Best Practices
--------------

Effective Communication
~~~~~~~~~~~~~~~~~~~~~~~

1. **Be Specific**: Provide context and specific details about your questions
2. **Share Code**: Include relevant code snippets for programming questions
3. **Ask Follow-ups**: Don't hesitate to ask for clarification or examples
4. **Use Commands**: Leverage built-in commands for better experience

Example of effective communication:

.. code-block:: text

   > I'm getting a "connection refused" error when trying to connect to my PostgreSQL 
     database from my Node.js application. Here's my connection code:
     
     ```javascript
     const { Pool } = require('pg');
     const pool = new Pool({
       user: 'myuser',
       host: 'localhost',
       database: 'mydb',
       password: 'mypass',
       port: 5432,
     });
     ```
     
     The database is running on Docker. What could be wrong?

Session Management
~~~~~~~~~~~~~~~~~~

- **Single Sessions**: Keep related topics in one session for better context
- **Clear When Needed**: Use ``/clear`` when switching to unrelated topics
- **Save Important Information**: Copy important responses before clearing
- **Regular Breaks**: Take breaks during long sessions to maintain focus

Privacy Considerations
~~~~~~~~~~~~~~~~~~~~~~

Remember that your conversations are sent to the configured AI provider:

- **Sensitive Data**: Avoid sharing passwords, API keys, or personal information
- **Code Review**: Be mindful when sharing proprietary code
- **Local Processing**: Consider local models for sensitive discussions

Troubleshooting Common Issues
-----------------------------

Connection Problems
~~~~~~~~~~~~~~~~~~~

If you encounter connection issues:

1. Check your internet connection
2. Verify API keys in configuration
3. Check provider status pages
4. Try switching models if available

.. code-block:: text

   > /status

This command helps diagnose connection issues.

Slow Responses
~~~~~~~~~~~~~~

If responses are slow:

- Check your internet connection
- Try a different model
- Verify provider service status
- Consider switching providers temporarily

Configuration Issues
~~~~~~~~~~~~~~~~~~~~

If settings aren't working:

1. Verify configuration file syntax
2. Check file permissions
3. Ensure API keys are valid
4. Review provider-specific settings

Next Steps
----------

Once you're comfortable with basic usage, explore:

- :doc:`advanced-features` - Learn about advanced Perspt features
- :doc:`providers` - Understand different AI providers and their capabilities
- :doc:`troubleshooting` - Comprehensive troubleshooting guide
- :doc:`../configuration` - Detailed configuration options
