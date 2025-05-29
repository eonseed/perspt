Advanced Features
=================

This guide covers Perspt's advanced features that enhance productivity and provide sophisticated interaction capabilities.

Configuration Profiles
-----------------------

Perspt supports multiple configuration profiles for different use cases:

Profile Management
~~~~~~~~~~~~~~~~~~

Create different profiles for various scenarios:

.. code-block:: bash

   # Work profile with conservative models
   perspt --config ~/.config/perspt/work.json
   
   # Creative profile with more experimental models
   perspt --config ~/.config/perspt/creative.json
   
   # Development profile with code-focused models
   perspt --config ~/.config/perspt/dev.json

Example profile configurations:

**Work Profile** (``work.json``):

.. code-block:: json

   {
     "provider": "openai",
     "model": "gpt-4",
     "max_tokens": 1000,
     "temperature": 0.3,
     "system_prompt": "You are a professional assistant focused on business and productivity.",
     "conversation_history_limit": 50
   }

**Creative Profile** (``creative.json``):

.. code-block:: json

   {
     "provider": "anthropic",
     "model": "claude-3-opus-20240229",
     "max_tokens": 2000,
     "temperature": 0.8,
     "system_prompt": "You are a creative assistant that helps with writing, brainstorming, and artistic projects.",
     "conversation_history_limit": 100
   }

Advanced Model Configuration
----------------------------

Temperature Control
~~~~~~~~~~~~~~~~~~~

Fine-tune response creativity and consistency:

.. code-block:: json

   {
     "temperature": 0.1,  // Very consistent, factual responses
     "temperature": 0.5,  // Balanced creativity and consistency
     "temperature": 0.9   // Highly creative, varied responses
   }

Top-p (Nucleus Sampling)
~~~~~~~~~~~~~~~~~~~~~~~~

Control response diversity:

.. code-block:: json

   {
     "top_p": 0.1,  // Very focused responses
     "top_p": 0.5,  // Moderate diversity
     "top_p": 0.9   // High diversity
   }

Frequency and Presence Penalties
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Reduce repetition and encourage novel content:

.. code-block:: json

   {
     "frequency_penalty": 0.5,  // Reduce repetition of frequent tokens
     "presence_penalty": 0.3    // Encourage discussing new topics
   }

Custom System Prompts
---------------------

Tailor AI behavior with system prompts:

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

   > /compare "Explain quantum computing" gpt-4 claude-3-opus

This sends the same prompt to multiple models and displays responses side by side.

Model Switching
~~~~~~~~~~~~~~~

Switch models mid-conversation while maintaining context:

.. code-block:: text

   > We've been discussing Python optimization
   AI: Yes, we covered several techniques including caching and algorithmic improvements.
   
   > /model claude-3-opus
   Model switched to claude-3-opus
   
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
