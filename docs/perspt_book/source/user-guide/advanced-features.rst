Advanced Features
=================

This guide covers Perspt's advanced features powered by the modern genai crate (v0.3.5), enabling sophisticated AI interactions, enhanced streaming capabilities, and productivity workflows.

Configuration Profiles and Multi-Provider Setup
-----------------------------------------------

GenAI-Powered Provider Management
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

With the genai crate integration, Perspt supports seamless switching between providers and models:

.. code-block:: bash

   # Work profile with reasoning models
   perspt --config ~/.config/perspt/work.json
   
   # Creative profile with latest models
   perspt --config ~/.config/perspt/creative.json
   
   # Development profile with coding-focused models
   perspt --config ~/.config/perspt/dev.json
   
   # Research profile with large context models
   perspt --config ~/.config/perspt/research.json

Example profile configurations:

**Work Profile** (``work.json``):

.. code-block:: json

   {
     "provider_type": "anthropic",
     "default_model": "claude-3-5-sonnet-20241022",
     "api_key": "${ANTHROPIC_API_KEY}",
     "providers": {
       "anthropic": "https://api.anthropic.com",
       "openai": "https://api.openai.com/v1"
     }
   }

**Creative Profile** (``creative.json``):

.. code-block:: json

   {
     "provider_type": "openai", 
     "default_model": "gpt-4.1",
     "api_key": "${OPENAI_API_KEY}",
     "providers": {
       "openai": "https://api.openai.com/v1",
       "xai": "https://api.x.ai/v1"
     }
   }

**Development Profile** (``dev.json``):

.. code-block:: json

   {
     "provider_type": "openai",
     "default_model": "o1-mini",
     "api_key": "${OPENAI_API_KEY}",
     "providers": {
       "openai": "https://api.openai.com/v1",
       "groq": "https://api.groq.com/openai/v1"
     }
   }

**Research Profile** (``research.json``):

.. code-block:: json

   {
     "provider_type": "google",
     "default_model": "gemini-1.5-pro",
     "api_key": "${GOOGLE_API_KEY}",
     "providers": {
       "google": "https://generativelanguage.googleapis.com",
       "anthropic": "https://api.anthropic.com"
     }
   }

Enhanced Streaming and Real-time Features
------------------------------------------

GenAI Crate Streaming Capabilities
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The genai crate provides sophisticated streaming with multiple event types:

**Standard Streaming**
- Token-by-token streaming for immediate feedback
- Smooth rendering with buffer management
- Context-aware response building

**Reasoning Model Streaming**
- ``ChatStreamEvent::Start``: Beginning of response
- ``ChatStreamEvent::Chunk``: Regular content tokens
- ``ChatStreamEvent::ReasoningChunk``: Thinking process (o1-series)
- ``ChatStreamEvent::End``: Response completion

**Advanced Streaming Features**

.. code-block:: text

   # Example with reasoning model (o1-mini)
   > Solve this complex math problem: ...
   
   [Reasoning] Let me think through this step by step...
   [Reasoning] First, I'll identify the key variables...
   [Reasoning] Now I'll apply the quadratic formula...
   [Streaming] Based on my analysis, the solution is...

**Real-time Model Switching**

Switch between models during conversations while maintaining context:

.. code-block:: bash

   # Start with fast model for exploration
   perspt --provider-type groq --model llama-3.1-8b-instant
   
   # Switch to reasoning model for complex analysis
   # (Context maintained across switch)
   perspt --provider-type openai --model o1-mini

Model Validation and Discovery
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Pre-flight model validation ensures reliable connections:

.. code-block:: bash

   # Validate model before starting conversation
   perspt --provider-type anthropic --model claude-3-5-sonnet-20241022 --list-models
   
   # Discover available models for provider
   perspt --provider-type google --list-models | grep gemini-2

**Automatic Fallback Configuration**

Configure automatic fallbacks for reliability:

.. code-block:: json

   {
     "provider_type": "openai",
     "default_model": "gpt-4o-mini",
     "fallback_providers": [
       {
         "provider_type": "anthropic",
         "model": "claude-3-5-haiku-20241022"
       },
       {
         "provider_type": "groq", 
         "model": "llama-3.1-70b-versatile"
       }
     ]
   }

Advanced Conversation Patterns
-------------------------------

Multi-Model Collaborative Workflows
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Leverage different models for their strengths within single sessions:

**Research and Analysis Workflow**

.. code-block:: bash

   # 1. Start with fast model for initial exploration
   perspt --provider-type groq --model llama-3.1-8b-instant
   
   # 2. Switch to reasoning model for deep analysis  
   perspt --provider-type openai --model o1-mini
   
   # 3. Use large context model for comprehensive review
   perspt --provider-type google --model gemini-1.5-pro

**Code Development Workflow**

.. code-block:: text

   # Use reasoning model for architecture planning
   > Design a microservices architecture for an e-commerce platform
   [Using o1-mini for complex reasoning]
   
   # Switch to coding-focused model for implementation
   > Now implement the user authentication service
   [Using claude-3-5-sonnet for code generation]
   
   # Use fast model for quick iterations and testing
   > Review this code for potential bugs
   [Using llama-3.1-70b for rapid feedback]

Provider-Specific Optimizations
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**OpenAI Reasoning Models**
- Best for: Complex problem-solving, mathematical reasoning, logic puzzles
- Features: Step-by-step thinking process, enhanced accuracy
- Usage: Allow extra time for reasoning, provide complex multi-step problems

**Anthropic Constitutional AI**
- Best for: Safety-critical applications, ethical reasoning, content moderation
- Features: Built-in safety guardrails, nuanced understanding
- Usage: Ideal for sensitive topics, business communications

**Google Multimodal Capabilities**
- Best for: Document analysis, image understanding, large context processing
- Features: 2M token context, multimodal input support
- Usage: Large document analysis, comprehensive research

**Groq Ultra-Fast Inference**
- Best for: Real-time chat, rapid prototyping, interactive sessions
- Features: Sub-second response times, consistent performance
- Usage: Brainstorming sessions, quick iterations

Local Model Privacy Features
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Enhanced privacy with local Ollama integration:

**Private Development Environment**

.. code-block:: json

   {
     "provider_type": "ollama",
     "default_model": "llama3.2:8b",
     "privacy_mode": true,
     "data_retention": "none",
     "providers": {
       "ollama": "http://localhost:11434"
     }
   }

**Sensitive Data Processing**

.. code-block:: bash

   # Use local models for proprietary code review
   perspt --provider-type ollama --model qwen2.5:14b
   
   # Process confidential documents offline
   perspt --provider-type ollama --model llama3.2:8b

Terminal UI Enhancements
~~~~~~~~~~~~~~~~~~~~~~~~

**Advanced Markdown Rendering**
- Syntax highlighting for code blocks
- Proper table formatting and alignment
- Mathematical equation rendering
- Nested list and quote support

**Streaming Visual Indicators**
- Real-time token streaming animations
- Reasoning process visualization for o1-models
- Connection status and model information
- Error recovery visual feedback

**Keyboard Shortcuts and Navigation**
- Input queuing while AI responds
- Seamless scrolling through long conversations  
- Context-aware copy/paste operations
- Quick model switching hotkeys

Domain Expert Prompts
~~~~~~~~~~~~~~~~~~~~~

**Software Development**:

.. code-block:: json

   {
     "system_prompt": "You are a senior software engineer with expertise in multiple programming languages, system design, and best practices. Provide detailed, practical advice with code examples when helpful. Focus on maintainability, performance, and security."
   }

**Academic Research**:

.. code-block:: json

   {
     "system_prompt": "You are an academic research assistant with expertise in methodology, citation practices, and critical analysis. Provide well-researched, evidence-based responses with appropriate academic tone and references when possible."
   }

**Creative Writing**:

.. code-block:: json

   {
     "system_prompt": "You are a creative writing mentor with expertise in storytelling, character development, and various literary forms. Help develop ideas, provide constructive feedback, and suggest techniques to improve writing craft."
   }

Context-Aware Prompts
~~~~~~~~~~~~~~~~~~~~~

Dynamic system prompts based on context:

.. code-block:: json

   {
     "system_prompt": "You are assisting with a ${PROJECT_TYPE} project. The user is working in ${LANGUAGE} and prefers ${STYLE} coding style. Adapt your responses accordingly and provide relevant examples."
   }

Session Persistence
-------------------

Save and Resume Conversations
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Perspt can maintain conversation history across sessions:

.. code-block:: json

   {
     "conversation_history": {
       "enabled": true,
       "max_sessions": 10,
       "auto_save": true,
       "storage_path": "~/.config/perspt/history/"
     }
   }

Session Commands
~~~~~~~~~~~~~~~~

Manage conversation sessions:

.. code-block:: text

   > /save session_name        # Save current conversation
   > /load session_name        # Load saved conversation
   > /list sessions           # List all saved sessions
   > /delete session_name     # Delete a saved session

Export Conversations
~~~~~~~~~~~~~~~~~~~

Export conversations in various formats:

.. code-block:: text

   > /export markdown conversation.md
   > /export json conversation.json
   > /export html conversation.html

Multi-Model Conversations
-------------------------

Model Comparison
~~~~~~~~~~~~~~~~

Compare responses from different models:

.. code-block:: text

   > /compare "Explain quantum computing" gpt-4o claude-3-5-sonnet-20241022

This sends the same prompt to multiple models and displays responses side by side.

Model Switching
~~~~~~~~~~~~~~~

Switch models mid-conversation while maintaining context:

.. code-block:: text

   > We've been discussing Python optimization
   AI: Yes, we covered several techniques including caching and algorithmic improvements.
   
   > /model claude-3-5-sonnet-20241022
   Model switched to claude-3-5-sonnet-20241022
   
   > Can you continue with memory optimization techniques?
   AI: Continuing our Python optimization discussion, let's explore memory optimization...

Plugin System
--------------

Perspt supports plugins for extended functionality:

Code Analysis Plugin
~~~~~~~~~~~~~~~~~~~

Analyze code quality and suggest improvements:

.. code-block:: json

   {
     "plugins": {
       "code_analysis": {
         "enabled": true,
         "languages": ["python", "javascript", "rust"],
         "features": ["linting", "security", "performance"]
       }
     }
   }

Usage:

.. code-block:: text

   > /analyze-code
   ```python
   def inefficient_function(data):
       result = []
       for item in data:
           if item > 0:
               result.append(item * 2)
       return result
   ```

Document Processing Plugin
~~~~~~~~~~~~~~~~~~~~~~~~~~

Process and analyze documents:

.. code-block:: json

   {
     "plugins": {
       "document_processor": {
         "enabled": true,
         "supported_formats": ["pdf", "docx", "txt", "md"],
         "max_file_size": "10MB"
       }
     }
   }

Usage:

.. code-block:: text

   > /process-document /path/to/document.pdf
   > Summarize this document and highlight key points

Web Integration Plugin
~~~~~~~~~~~~~~~~~~~~~~

Fetch and analyze web content:

.. code-block:: json

   {
     "plugins": {
       "web_integration": {
         "enabled": true,
         "allowed_domains": ["github.com", "stackoverflow.com", "docs.python.org"],
         "max_content_length": 50000
       }
     }
   }

Usage:

.. code-block:: text

   > /fetch-url https://docs.python.org/3/library/asyncio.html
   > Explain the key concepts from this documentation

Advanced Conversation Patterns
-------------------------------

Role-Playing Scenarios
~~~~~~~~~~~~~~~~~~~~~~

Set up specific roles for focused assistance:

.. code-block:: text

   > /role code_reviewer
   AI: I'm now acting as a code reviewer. Please share your code for detailed analysis.
   
   > /role system_architect
   AI: I'm now acting as a system architect. Let's discuss your system design requirements.

Collaborative Problem Solving
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Break down complex problems into manageable parts:

.. code-block:: text

   > /problem-solving mode
   AI: I'm now in problem-solving mode. Let's break down your challenge systematically.
   
   > I need to design a scalable microservices architecture
   AI: Great! Let's approach this systematically:
       1. First, let's identify your core business domains
       2. Then we'll determine service boundaries
       3. Next, we'll design the communication patterns
       4. Finally, we'll address scalability and deployment
       
       Let's start with step 1: What are your main business domains?

Iterative Refinement
~~~~~~~~~~~~~~~~~~~~

Continuously improve solutions through iteration:

.. code-block:: text

   > /iterative mode
   AI: I'm now in iterative mode. I'll help you refine solutions step by step.
   
   > Here's my initial algorithm implementation
   AI: I see several areas for improvement. Let's iterate:
       Version 1: Your current implementation
       Version 2: Optimized algorithm complexity
       Version 3: Added error handling
       Version 4: Improved readability and maintainability
       
       Which aspect would you like to focus on first?

Automation and Scripting
------------------------

Command Scripting
~~~~~~~~~~~~~~~~~

Create scripts for common workflows:

**development_workflow.perspt**:

.. code-block:: text

   /model gpt-4
   /role senior_developer
   /context "Working on a ${PROJECT_NAME} project in ${LANGUAGE}"
   
   Ready for development assistance!

Run with:

.. code-block:: bash

   perspt --script development_workflow.perspt

Batch Processing
~~~~~~~~~~~~~~~

Process multiple queries in batch:

.. code-block:: text

   > /batch process_queries.txt

Where ``process_queries.txt`` contains:

.. code-block:: text

   Explain the benefits of microservices
   ---
   Compare REST vs GraphQL APIs
   ---
   Best practices for database design

Configuration Validation
-------------------------

Validate your configuration setup:

.. code-block:: text

   > /validate-config

This checks:

- API key validity
- Model availability
- Configuration syntax
- Plugin compatibility
- Network connectivity

Performance Optimization
------------------------

Response Caching
~~~~~~~~~~~~~~~

Cache responses for repeated queries:

.. code-block:: json

   {
     "cache": {
       "enabled": true,
       "ttl": 3600,
       "max_size": "100MB",
       "strategy": "lru"
     }
   }

Parallel Processing
~~~~~~~~~~~~~~~~~~

Process multiple requests simultaneously:

.. code-block:: json

   {
     "parallel_processing": {
       "enabled": true,
       "max_concurrent": 3,
       "timeout": 30
     }
   }

Custom Integrations
-------------------

IDE Integration
~~~~~~~~~~~~~~

Integrate Perspt with your development environment:

**VS Code Extension**:

.. code-block:: json

   {
     "vscode": {
       "enabled": true,
       "keybindings": {
         "ask_perspt": "Ctrl+Shift+P",
         "explain_code": "Ctrl+Shift+E"
       }
     }
   }

**Vim Plugin**:

.. code-block:: vim

   " Add to .vimrc
   nnoremap <leader>p :!perspt --query "<C-R><C-W>"<CR>

API Integration
~~~~~~~~~~~~~~

Use Perspt programmatically:

.. code-block:: python

   import requests
   
   def ask_perspt(question):
       response = requests.post('http://localhost:8080/api/chat', {
           'message': question,
           'model': 'gpt-4'
       })
       return response.json()['response']

Next Steps
----------

Explore more advanced topics:

- :doc:`providers` - Deep dive into AI provider capabilities
- :doc:`troubleshooting` - Advanced troubleshooting techniques
- :doc:`../developer-guide/extending` - Create custom plugins and extensions
- :doc:`../api/index` - API reference for programmatic usage
