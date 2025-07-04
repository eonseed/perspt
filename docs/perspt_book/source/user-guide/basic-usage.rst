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
   perspt --provider-type gemini --model gemini-1.5-flash

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

   # Interface Modes  
   perspt --simple-cli              # Enable simple CLI mode
   perspt --log-file FILE           # Session logging (requires --simple-cli)

**Supported Provider Types**

.. code-block:: bash

   openai          # OpenAI GPT models (default)
   anthropic       # Anthropic Claude models  
   google          # Google Gemini models
   groq            # Groq ultra-fast inference
   cohere          # Cohere Command models
   xai             # XAI Grok models
   deepseek        # DeepSeek models
   ollama          # Local Ollama models

**Example Usage Patterns**

.. code-block:: bash

   # TUI mode with specific provider
   perspt -p openai -m gpt-4o-mini
   
   # Simple CLI mode with logging
   perspt --simple-cli --log-file session.txt
   
   # Creative writing with Claude in simple CLI
   perspt --simple-cli -p anthropic -m claude-3-5-sonnet-20241022
   
   # Fast local inference in simple CLI
   perspt --simple-cli -p ollama -m llama3.2
   
   # Validate model before starting
   perspt -p google -m gemini-2.0-flash-exp --list-models

Interactive Commands
--------------------

Once in the chat interface, you can use keyboard shortcuts and built-in commands for efficient interaction:

**Built-in Chat Commands**

.. list-table::
   :widths: 30 70
   :header-rows: 1

   * - Command
     - Description
   * - ``/save``
     - Save conversation with timestamped filename (e.g., conversation_1735123456.txt)
   * - ``/save filename.txt``
     - Save conversation with custom filename

**Conversation Export Examples**

.. code-block:: text

   # Save with automatic timestamped filename
   > /save
   💾 Conversation saved to: conversation_1735123456.txt
   
   # Save with custom filename for organization
   > /save python_debugging_session.txt
   💾 Conversation saved to: python_debugging_session.txt
   
   # Attempt to save empty conversation
   > /save
   ❌ No conversation to save

**Export File Format**

The saved conversations are exported as plain text files with the following structure:

.. code-block:: text

   Perspt Conversation
   ==================
   [2024-01-01 12:00:00] User: Hello, can you explain quantum computing?
   [2024-01-01 12:00:01] Assistant: Quantum computing is a revolutionary approach...

   [2024-01-01 12:02:15] User: What are the main applications?
   [2024-01-01 12:02:16] Assistant: The main applications of quantum computing include...

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

Testing Local Models with Ollama
---------------------------------

Ollama provides an excellent way to test local models without API keys or internet connectivity. This section walks through setting up and testing Ollama with Perspt.

Prerequisites
~~~~~~~~~~~~~

**Install and Start Ollama**

.. code-block:: bash

   # macOS
   brew install ollama
   
   # Linux
   curl -fsSL https://ollama.ai/install.sh | sh
   
   # Start Ollama service
   ollama serve

**Download Test Models**

.. code-block:: bash

   # Download Llama 3.2 (3B) - fast and efficient
   ollama pull llama3.2
   
   # Download Code Llama - for coding tasks
   ollama pull codellama
   
   # Verify models are available
   ollama list

Basic Ollama Testing
~~~~~~~~~~~~~~~~~~~~

**Start with Simple Conversations**

.. code-block:: bash

   # Test basic functionality
   perspt --provider-type ollama --model llama3.2

Example conversation flow:

.. code-block:: text

   Perspt v0.4.0 - Performance LLM Chat CLI
   Provider: Ollama | Model: llama3.2 | Status: Connected ✓
   Local model hosting - no API key required
   
   > Hello! Can you help me understand how LLMs work?
   
   [Assistant responds with explanation of language models...]
   
   > That's helpful! Now explain it like I'm 5 years old.
   
   [Assistant provides simplified explanation...]

**Test Different Model Types**

.. code-block:: bash

   # General conversation
   perspt --provider-type ollama --model llama3.2
   
   # Coding assistance
   perspt --provider-type ollama --model codellama
   
   # Larger model for complex tasks (if you have enough RAM)
   perspt --provider-type ollama --model llama3.1:8b

Performance Testing
~~~~~~~~~~~~~~~~~~~

**Model Comparison**

Test different model sizes to find the right balance for your system:

.. list-table::
   :header-rows: 1
   :widths: 25 20 25 30

   * - Model
     - Size
     - RAM Required
     - Best For
   * - ``llama3.2``
     - 3B
     - ~4GB
     - Quick responses, chat
   * - ``llama3.1:8b``
     - 8B
     - ~8GB
     - Better reasoning, longer context
   * - ``codellama``
     - 7B
     - ~7GB
     - Code generation, technical tasks
   * - ``mistral``
     - 7B
     - ~7GB
     - Balanced performance

**Speed Testing**

.. code-block:: bash

   # Time how long responses take
   time perspt --provider-type ollama --model llama3.2

   # Compare with cloud providers
   time perspt --provider-type openai --model gpt-4o-mini

**Practical Test Scenarios**

.. code-block:: text

   # Test 1: Basic Knowledge
   > What is the capital of France?
   
   # Test 2: Reasoning
   > If a train travels 60 mph for 2.5 hours, how far does it go?
   
   # Test 3: Creative Writing
   > Write a short story about a robot learning to paint.
   
   # Test 4: Code Generation (with codellama)
   > Write a Python function to calculate fibonacci numbers.

Troubleshooting Ollama
~~~~~~~~~~~~~~~~~~~~~~

**Common Issues**

.. code-block:: bash

   # Check if Ollama is running
   curl http://localhost:11434/api/tags
   
   # If connection fails
   ollama serve
   
   # List available models
   perspt --provider-type ollama --list-models
   
   # Pull missing models
   ollama pull llama3.2

**Performance Issues**

- **Slow responses**: Try smaller models (llama3.2 vs llama3.1:8b)
- **Out of memory**: Close other applications or use lighter models
- **Model not found**: Ensure you've pulled the model with ``ollama pull``

**Configuration for Regular Use**

Create a config file for easy Ollama usage:

.. code-block:: json

   {
     "provider_type": "ollama",
     "default_model": "llama3.2",
     "providers": {
       "ollama": "http://localhost:11434/v1"
     },
     "api_key": "not-required"
   }

.. code-block:: bash

   # Save as ollama_config.json and use
   perspt --config ollama_config.json

**Benefits of Local Testing**

- **Privacy**: All data stays on your machine
- **Cost**: No API fees or usage limits
- **Offline**: Works without internet after initial setup
- **Experimentation**: Try different models and settings freely
- **Learning**: Understand model capabilities and limitations

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

Simple CLI Mode - Direct Q&A Interface
----------------------------------------

**NEW in v0.4.5**: Perspt now includes a minimal command-line interface mode for direct question-and-answer interaction without the TUI overlay. This mode follows the Unix philosophy of simple, composable tools and is perfect for scripting, accessibility needs, or users who prefer command-line interfaces.

When to Use Simple CLI Mode
~~~~~~~~~~~~~~~~~~~~~~~~~~~

The simple CLI mode is ideal for:

- **🤖 Scripting & Automation**: Integrate Perspt into shell scripts, CI/CD pipelines, or automated workflows
- **♿ Accessibility**: Simple, scrolling console output for users with screen readers or accessibility needs
- **📝 Logging & Documentation**: Built-in session logging for keeping detailed records of AI interactions
- **⚡ Quick Queries**: Lightweight interface for fast, one-off questions without UI overhead  
- **🐧 Unix Philosophy**: Clean, composable tool that works well with pipes, redirects, and other command-line tools

Basic Simple CLI Usage
~~~~~~~~~~~~~~~~~~~~~~

**Starting Simple CLI Mode**

.. code-block:: bash

   # Basic simple CLI mode (uses auto-detected provider)
   perspt --simple-cli

   # With specific provider and model
   perspt --simple-cli --provider-type openai --model gpt-4o-mini

   # With Gemini
   perspt --simple-cli --provider-type gemini --model gemini-1.5-flash

   # With local Ollama (no API key needed)
   perspt --simple-cli --provider-type ollama --model llama3.2

**Interactive Session Example**

.. code-block:: text

   $ perspt --simple-cli --provider-type openai --model gpt-4o-mini
   Perspt Simple CLI Mode
   Model: gpt-4o-mini
   Type 'exit' or press Ctrl+D to quit.

   > What is the capital of France?
   Paris is the capital and largest city of France. It's located in the 
   north-central part of the country on the Seine River...

   > How many people live there?
   The city of Paris proper has a population of approximately 2.1 million 
   people as of recent estimates. However, the Greater Paris metropolitan 
   area (Île-de-France region) has a much larger population...

   > exit
   Goodbye!

Session Logging
~~~~~~~~~~~~~~~

One of the key features of simple CLI mode is built-in session logging:

**Basic Logging**

.. code-block:: bash

   # Log entire session to a file
   perspt --simple-cli --log-file my-session.txt

   # Use timestamped filenames for organization
   perspt --simple-cli --log-file "$(date +%Y%m%d_%H%M%S)_ai_session.txt"

   # Combined with specific provider
   perspt --simple-cli --provider-type anthropic --model claude-3-5-sonnet-20241022 --log-file claude-session.txt

**Log File Format**

The log files contain both user input and AI responses in a clean, readable format:

.. code-block:: text

   > What is machine learning?
   Machine learning is a subset of artificial intelligence (AI) that involves 
   training algorithms to recognize patterns in data and make predictions or 
   decisions without being explicitly programmed for each specific task...

   > Give me 3 practical examples
   Here are three practical examples of machine learning in everyday use:

   1. **Email Spam Detection**: Email services like Gmail use machine learning...
   2. **Recommendation Systems**: Platforms like Netflix, Spotify, and Amazon...
   3. **Voice Assistants**: Siri, Alexa, and Google Assistant use machine learning...

   > 

Scripting and Automation
~~~~~~~~~~~~~~~~~~~~~~~~

The simple CLI mode excels at scripting and automation scenarios:

**Direct Input via Pipes**

.. code-block:: bash

   # Pipe a single question
   echo "What is quantum computing?" | perspt --simple-cli

   # Use in shell scripts
   #!/bin/bash
   question="Explain the difference between REST and GraphQL APIs"
   echo "$question" | perspt --simple-cli --log-file api-explanation.txt

**Multiple Questions**

.. code-block:: bash

   # Chain multiple questions with automatic exit
   {
     echo "What is Docker?"
     echo "How is it different from virtual machines?"
     echo "Give me a simple Docker example"
     echo "exit"
   } | perspt --simple-cli --log-file docker-tutorial.txt

**Environment Integration**

.. code-block:: bash

   # Set up environment for regular use
   export OPENAI_API_KEY="your-key"
   alias ai="perspt --simple-cli"
   alias ai-log="perspt --simple-cli --log-file"

   # Now use anywhere
   ai
   ai-log research-session.txt

   # Add to your .bashrc or .zshrc for permanent setup
   echo 'alias ai="perspt --simple-cli"' >> ~/.bashrc

Advanced Simple CLI Features
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Error Handling**

Unlike the TUI mode, simple CLI mode is designed to be resilient for scripting:

.. code-block:: text

   > This might cause an error
   Error: Rate limit exceeded. Please try again in a few moments.

   > This question works fine
   [Normal response continues...]

   > exit
   Goodbye!

Individual request errors don't terminate the session, making it suitable for long-running scripts.

**Exit Methods**

Simple CLI mode supports multiple exit methods for different use cases:

.. code-block:: bash

   # Method 1: Type 'exit' command
   > exit

   # Method 2: Send EOF (Ctrl+D on Unix, Ctrl+Z on Windows)
   > ^D

   # Method 3: Interrupt signal (Ctrl+C)
   > ^C

**Configuration Files**

Create dedicated configuration files for simple CLI use:

.. code-block:: json

   {
     "provider_type": "openai",
     "default_model": "gpt-4o-mini",
     "api_key": "your-api-key"
   }

.. code-block:: bash

   # Save as simple-cli-config.json and use
   perspt --simple-cli --config simple-cli-config.json

Simple CLI vs TUI Mode Comparison
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 30 35 35

   * - Feature
     - Simple CLI Mode
     - TUI Mode
   * - **Interface Style**
     - Minimal Unix prompt
     - Rich terminal UI
   * - **Scrolling**
     - Natural terminal scrolling
     - Built-in history navigation
   * - **Markdown Rendering**
     - Raw text output
     - Formatted rendering
   * - **Session Management**
     - Built-in logging option
     - Manual ``/save`` command
   * - **Scripting Support**
     - Excellent (pipes, redirects)
     - Not suitable
   * - **Accessibility**
     - High (screen reader friendly)
     - Moderate
   * - **Resource Usage**
     - Minimal overhead
     - Moderate (UI rendering)
   * - **Background Operation**
     - Foreground only
     - Visual feedback
   * - **Multi-line Input**
     - Line-by-line
     - Rich text editing

Use Case Examples
~~~~~~~~~~~~~~~~

**Documentation Generation**

.. code-block:: bash

   # Generate documentation for a project
   {
     echo "Explain the architecture of a REST API"
     echo "What are the best practices for REST API design?"
     echo "How do you handle authentication in REST APIs?"
     echo "exit"
   } | perspt --simple-cli --log-file rest-api-docs.txt

**Code Review Assistant**

.. code-block:: bash

   # Review code with AI assistance
   {
     echo "Review this Python function for potential issues:"
     cat my_function.py
     echo "exit"
   } | perspt --simple-cli --provider-type openai --model gpt-4o --log-file code-review.txt

**Learning and Research**

.. code-block:: bash

   # Research session with logging
   perspt --simple-cli --provider-type anthropic --model claude-3-5-sonnet-20241022 \
         --log-file "$(date +%Y%m%d)_learning_session.txt"

**Quick Consultations**

.. code-block:: bash

   # Quick question without UI overhead
   echo "What's the best way to optimize PostgreSQL queries?" | perspt --simple-cli

Troubleshooting Simple CLI Mode
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Common Issues**

.. code-block:: bash

   # Test if simple CLI mode works
   perspt --simple-cli --provider-type openai --list-models

   # Verify logging permissions
   touch test-log.txt && rm test-log.txt

   # Check if provider is properly configured
   perspt --simple-cli --provider-type your-provider --model your-model

**Performance Tips**

- Use faster models like ``gpt-4o-mini`` or ``gemini-1.5-flash`` for quick queries
- For local usage, ``ollama`` with ``llama3.2`` provides excellent performance
- Log files are appended to, so you can continue sessions across multiple runs

**Integration with Other Tools**

.. code-block:: bash

   # Use with jq for structured output (if AI returns JSON)
   echo "Return the top 3 programming languages as JSON" | \
   perspt --simple-cli | jq '.languages[]'

   # Use with grep for filtering
   echo "List 10 Linux commands" | perspt --simple-cli | grep -E "^[0-9]+"

   # Combine with watch for monitoring
   watch -n 300 'echo "What is the current status of the Python package index?" | perspt --simple-cli'
