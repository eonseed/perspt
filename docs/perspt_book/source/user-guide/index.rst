.. _user-guide:

User Guide
==========

This comprehensive user guide covers everything you need to know to use Perspt effectively, from basic conversations to advanced productivity techniques.

.. toctree::
   :maxdepth: 2
   :caption: User Guide Contents

   basic-usage
   advanced-features
   providers
   troubleshooting

Overview
--------

Perspt is a high-performance terminal-based chat application built with Rust that provides a unified interface for interacting with multiple Large Language Model (LLM) providers. Using the modern `genai` crate (v0.3.5), Perspt offers seamless access to the latest AI models with enhanced streaming capabilities, robust error handling, and intuitive CLI functionality.

**Key Features:**

- **Multi-Provider Support**: OpenAI, Anthropic, Google, Groq, Cohere, XAI, and Ollama
- **Latest Models**: Support for reasoning models (o1-mini, o1-preview, o3-mini), GPT-4.1, Claude 3.5, Gemini 2.5 Pro
- **Real-time Streaming**: Enhanced streaming with proper reasoning chunk handling
- **Robust CLI**: Working command-line arguments with model validation
- **Beautiful UI**: Responsive terminal interface with markdown rendering

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: 🚀 Basic Usage
      :link: basic-usage
      :link-type: doc

      Learn the fundamentals of chatting with AI models, keyboard shortcuts, and everyday usage patterns.

   .. grid-item-card:: ⚡ Advanced Features
      :link: advanced-features
      :link-type: doc

      Discover powerful features like input queuing, markdown rendering, and productivity workflows.

   .. grid-item-card:: 🔀 Provider Guide
      :link: providers
      :link-type: doc

      Complete guide to all supported AI providers, their models, and specific configuration options.

   .. grid-item-card:: 🛠️ Troubleshooting
      :link: troubleshooting
      :link-type: doc

      Solutions to common issues, error messages, and optimization tips.

Quick Reference
---------------

Essential Keyboard Shortcuts
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. list-table::
   :widths: 25 75
   :header-rows: 1

   * - Shortcut
     - Action
   * - **Enter**
     - Send message
   * - **Ctrl+C**
     - Exit application
   * - **↑/↓ Keys**
     - Scroll chat history
   * - **Page Up/Down**
     - Fast scroll
   * - **Ctrl+L**
     - Clear screen

Common Commands
~~~~~~~~~~~~~~~

.. code-block:: bash

   # Start with default settings (gpt-4o-mini)
   perspt

   # Use specific model with validation
   perspt --model gpt-4.1

   # Switch provider and model
   perspt --provider-type anthropic --model claude-3-5-sonnet-20241022

   # List available models for current provider
   perspt --list-models

   # Use custom configuration file
   perspt --config my-config.json

   # Override API key from command line
   perspt --api-key your-api-key

   # Use provider profile from config
   perspt --provider work-profile

Typical Workflows
-----------------

Daily Development
~~~~~~~~~~~~~~~~~

1. **Code Review**: Paste code and ask for feedback
2. **Documentation**: Generate or improve documentation
3. **Debugging**: Discuss error messages and solutions
4. **Learning**: Ask about new technologies or concepts

Research and Writing
~~~~~~~~~~~~~~~~~~~~

1. **Information Gathering**: Ask questions about topics
2. **Content Creation**: Get help with writing and editing
3. **Brainstorming**: Generate ideas and explore concepts
4. **Fact Checking**: Verify information and get references

Getting the Most from Perspt
-----------------------------

Best Practices
~~~~~~~~~~~~~~

- **Be Specific**: Clear, detailed questions get better answers
- **Provide Context**: Include relevant background information
- **Iterate**: Build on previous responses for deeper understanding
- **Experiment**: Try different models for different types of tasks

Productivity Tips
~~~~~~~~~~~~~~~~~

- **Use Configuration Files**: Set up profiles for different use cases
- **Learn Keyboard Shortcuts**: Speed up your workflow
- **Leverage Streaming**: Keep typing while AI responds
- **Save Important Conversations**: Copy valuable responses

What's Next?
------------

Choose your path based on your experience level:

**New Users**: Start with :doc:`basic-usage` to learn the fundamentals.

**Experienced Users**: Jump to :doc:`advanced-features` for productivity techniques.

**Multi-Provider Users**: Check out :doc:`providers` for provider-specific tips.

**Having Issues?**: Visit :doc:`troubleshooting` for solutions.

.. seealso::

   - :doc:`../getting-started` - Initial setup and first conversation
   - :doc:`../configuration` - Customizing Perspt for your workflow
   - :doc:`../developer-guide/index` - Contributing and extending Perspt
