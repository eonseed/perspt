Basic Usage
===========

This guide covers the fundamental usage patterns of Perspt, from starting your first conversation to understanding the CLI commands and streaming features powered by the modern genai crate.

Starting Perspt
----------------

Perspt uses the latest genai crate (v0.3.5) for unified LLM access with enhanced capabilities. You can start it with various configuration options:

**Basic Usage**

.. code-block:: bash

   # Start with default configuration (OpenAI gpt-4o-mini)
   perspt

**Provider Selection**

.. code-block:: bash

   # Use Anthropic with Claude 3.5 Sonnet
   perspt --provider-type anthropic --model claude-3-5-sonnet-20241022

   # Use Google Gemini
   perspt --provider-type google --model gemini-1.5-flash

   # Use latest reasoning models
   perspt --provider-type openai --model o1-mini

**Configuration Files**

.. code-block:: bash

   # Use custom configuration file
   perspt --config /path/to/your/config.json

   # Override API key from command line
   perspt --api-key your-api-key-here

**Model Discovery**

.. code-block:: bash

   # List all available models for current provider
   perspt --list-models

   # List models for specific provider
   perspt --provider-type anthropic --list-models

Your First Conversation
------------------------

When Perspt starts, you'll see a clean interface with model validation and streaming capabilities:

.. code-block:: text

   Perspt v0.4.0 - Performance LLM Chat CLI
   Provider: OpenAI | Model: gpt-4o-mini | Status: Connected ✓
   Enhanced streaming with genai crate v0.3.5
   
   Type your message and press Enter to start a conversation.
   Use Ctrl+C to exit gracefully.
   
   > 

Simply type your message or question and press Enter. Perspt will validate the model connection before starting:

.. code-block:: text

   > Hello, can you explain quantum computing?

**Enhanced Streaming Experience**

With the genai crate integration, responses stream in real-time with proper event handling:

- **Reasoning Models**: See thinking process with reasoning chunks for o1-series models
- **Regular Models**: Smooth token-by-token streaming for immediate feedback  
- **Error Recovery**: Robust error handling with terminal restoration

The AI maintains context throughout the session and provides rich, formatted responses with markdown support.

CLI Arguments and Options
-------------------------

Perspt supports comprehensive command-line arguments that actually work with the genai crate integration:

**Core Arguments**

.. code-block:: bash

   # Configuration
   perspt --config|-c FILE          # Custom configuration file path
   
   # Authentication  
   perspt --api-key|-k KEY          # Override API key
   
   # Model Selection
   perspt --model|-m MODEL          # Specific model name
   perspt --provider-type|-p TYPE   # Provider type
   perspt --provider PROFILE        # Provider profile from config
   
   # Discovery
   perspt --list-models|-l          # List available models

**Supported Provider Types**

.. code-block:: bash

   openai          # OpenAI GPT models (default)
   anthropic       # Anthropic Claude models  
   google          # Google Gemini models
   groq            # Groq ultra-fast inference
   cohere          # Cohere Command models
   xai             # XAI Grok models
   ollama          # Local Ollama models
   mistral         # Mistral AI models
   perplexity      # Perplexity models
   deepseek        # DeepSeek models
   aws-bedrock     # AWS Bedrock service
   azure-openai    # Azure OpenAI service

**Example Usage Patterns**

.. code-block:: bash

   # Quick reasoning with o1-mini
   perspt -p openai -m o1-mini
   
   # Creative writing with Claude
   perspt -p anthropic -m claude-3-5-sonnet-20241022
   
   # Fast local inference  
   perspt -p ollama -m llama3.2
   
   # Validate model before starting
   perspt -p google -m gemini-2.0-flash-exp --list-models

Interactive Commands
--------------------

Once in the chat interface, you can use keyboard shortcuts for efficient interaction:

**Navigation Shortcuts**

.. list-table::
   :widths: 25 75
   :header-rows: 1

   * - Shortcut
     - Action
   * - **Enter**
     - Send message (validated before transmission)
   * - **Ctrl+C**
     - Exit gracefully with terminal restoration
   * - **↑/↓ Keys**
     - Scroll through chat history  
   * - **Page Up/Down**
     - Fast scroll through long conversations
   * - **Ctrl+L**
     - Clear screen (preserves context)

**Input Management**

- **Multi-line Input**: Natural line breaks supported
- **Input Queuing**: Type new messages while AI responds  
- **Context Preservation**: Full conversation history maintained
- **Markdown Rendering**: Rich text formatting in responses

Managing Conversations
----------------------

Enhanced Context Management
~~~~~~~~~~~~~~~~~~~~~~~~~~~

With the genai crate integration, Perspt provides superior context handling:

**Context Awareness**
- Full conversation history maintained per session
- Automatic context window management for each provider
- Smart truncation when approaching token limits
- Provider-specific optimizations

**Streaming and Responsiveness**
- Real-time token streaming for immediate feedback
- Reasoning chunk display for o1-series models  
- Background processing while you type new queries
- Robust error recovery with terminal restoration

Example of enhanced conversation flow:

.. code-block:: text

   > I'm working on a Rust project with async/await
   [Streaming...] I'd be happy to help with your Rust async project! 
   Rust's async/await provides excellent performance for concurrent operations...
   
   > How do I handle multiple futures concurrently?
   [Streaming...] For handling multiple futures concurrently in your Rust project,
   you have several powerful options with tokio...
   
   > Show me an example with tokio::join!
   [Reasoning...] Let me provide a practical example using tokio::join!
   for your async Rust project...

**Advanced Conversation Features**

- **Input Queuing**: Continue typing while AI generates responses
- **Context Preservation**: Seamless topic transitions within sessions  
- **Error Recovery**: Automatic reconnection and state restoration
- **Model Validation**: Pre-flight checks ensure model availability

Message Formatting and Rendering
---------------------------------

Enhanced Markdown Support
~~~~~~~~~~~~~~~~~~~~~~~~~

Perspt includes a custom markdown parser optimized for terminal rendering:

**Supported Formatting**

.. code-block:: text

   **Bold text** and *italic text*
   `inline code` and ```code blocks```
   
   # Headers and ## Subheaders
   
   - Bullet points
   - With proper indentation
   
   1. Numbered lists  
   2. With automatic formatting

**Code Block Rendering**

Share code with syntax highlighting hints:

.. code-block:: text

   > Can you help optimize this Rust function?
   
   ```rust
   async fn process_data(data: Vec<String>) -> Result<Vec<String>, Error> {
       // Your code here
   }
   ```

**Long Message Handling**

- Automatic text wrapping for terminal width
- Proper paragraph breaks and spacing
- Smooth scrolling through long responses
- Visual indicators for streaming progress

Best Practices for Effective Usage
-----------------------------------

Communication Strategies
~~~~~~~~~~~~~~~~~~~~~~~~

**Optimized for GenAI Crate Integration**

1. **Model-Specific Approaches**:
   - **Reasoning Models (o1-series)**: Provide complex problems and let them work through the logic
   - **Fast Models (gpt-4o-mini, claude-3-haiku)**: Use for quick questions and iterations
   - **Large Context Models (claude-3-5-sonnet)**: Share entire codebases or documents

2. **Provider Strengths**:
   - **OpenAI**: Latest reasoning capabilities, coding assistance
   - **Anthropic**: Safety-focused, analytical reasoning, constitutional AI
   - **Google**: Multimodal capabilities, large context windows
   - **Groq**: Ultra-fast inference for real-time conversations

**Effective Prompting Techniques**

.. code-block:: text

   # Instead of vague requests:
   > Help me with my code
   
   # Be specific with context:
   > I'm working on a Rust HTTP server using tokio and warp. The server 
   compiles but panics when handling concurrent requests. Here's the 
   relevant code: [paste code]. Can you help me identify the race condition?

**Session Management Strategies**

- **Single-Topic Sessions**: Keep related discussions in one session for better context
- **Model Switching**: Use `perspt --list-models` to explore optimal models for different tasks
- **Configuration Profiles**: Set up different configs for work, creative, and development tasks

Troubleshooting Common Issues
-----------------------------

Connection and Model Issues
~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Model Validation Failures**

.. code-block:: bash

   # Check if model exists for provider
   perspt --provider-type openai --list-models | grep o1-mini
   
   # Test connection with basic model
   perspt --provider-type openai --model gpt-3.5-turbo

**API Key Problems**

.. code-block:: bash

   # Test API key directly
   perspt --api-key your-key --provider-type openai --list-models
   
   # Use environment variables (recommended)
   export OPENAI_API_KEY="your-key"
   perspt

**Streaming Issues**

If streaming responses seem slow or interrupted:

1. **Network Check**: Ensure stable internet connection
2. **Provider Status**: Check provider service status pages  
3. **Model Selection**: Try faster models like gpt-4o-mini
4. **Terminal Compatibility**: Ensure terminal supports ANSI colors and UTF-8

Performance Optimization
~~~~~~~~~~~~~~~~~~~~~~~~

**Memory and Speed**

- **Local Models**: Use Ollama for privacy and reduced latency
- **Model Selection**: Choose appropriate model size for your task
- **Context Management**: Clear context for unrelated new topics

**Cost Optimization**

- **Model Tiers**: Use cheaper models (gpt-3.5-turbo) for simple queries
- **Streaming Benefits**: Stop generation early if you have enough information
- **Batch Questions**: Ask related questions in single sessions to share context

Next Steps
----------

Once you're comfortable with basic usage:

- **Advanced Features**: Learn about configuration profiles and system prompts in :doc:`advanced-features`
- **Provider Deep-Dive**: Explore specific provider capabilities in :doc:`providers`  
- **Troubleshooting**: Get help with specific issues in :doc:`troubleshooting`
- **Configuration**: Set up custom configurations in :doc:`../configuration`
