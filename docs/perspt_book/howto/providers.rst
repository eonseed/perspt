.. _howto-providers:

Set Up Providers
================

**Choose the adapter, set its credentials, then select an explicit route.** The :doc:`../user-guide/providers` table lists all supported adapters and their environment variables.
Replace ``<model-id>`` in these examples with a model available to the account.

Cloud API keys
--------------

Set the key for the provider we intend to use:

.. code-block:: bash

   export GEMINI_API_KEY="<api-key>"
   perspt chat --model 'gemini::<model-id>'

   # Alternatives
   export OPENAI_API_KEY="<api-key>"
   perspt chat --model 'openai::<model-id>'

   export ANTHROPIC_API_KEY="<api-key>"
   perspt chat --model 'anthropic::<model-id>'

The same route syntax applies to ``groq``, ``cohere``, ``xai``, and ``deepseek`` with their corresponding credentials.
Model names in examples elsewhere in the book illustrate configuration; they do not establish current account access or agent tool support.

Vertex AI
---------

Configure local Application Default Credentials and the project:

.. code-block:: bash

   gcloud auth application-default login
   export VERTEX_PROJECT_ID="<project-id>"
   export VERTEX_LOCATION="global"
   perspt chat --model 'vertex::<model-id>'

Perspt resolves tokens per request through ADC.
An optional ``VERTEX_API_KEY`` supplies an explicit bearer-token override.
The location variable is ``VERTEX_LOCATION``.

Ollama
------

Start ``ollama serve`` in another terminal, then install and select a model:

.. code-block:: bash

   ollama pull llama3.2
   perspt chat --model ollama::llama3.2

No cloud key is needed.
An explicit prefix keeps this route local even when cloud keys are present.
Use ``ollama list`` to see installed model IDs.
Agent mode additionally needs structured tool-calling support.

Several providers in one agent run
----------------------------------

.. code-block:: toml

   [providers.cloud]
   adapter = "openai"
   api_key_env = "OPENAI_API_KEY"

   [providers.local]
   adapter = "ollama"
   base_url = "http://localhost:11434"

   [models]
   actuator = "cloud::<coding-model-id>"
   speculator = "local::<explorer-model-id>"

The ``speculator`` entry supplies the explorer fallback route.
We replace both model placeholders with available models and can override roles per run with ``--actuator-model`` and ``--explorer-model``.

Check the connection
--------------------

.. code-block:: bash

   perspt providers
   perspt providers --probe

The second command makes live model requests.
A model that supports ordinary chat may still lack the structured operations needed by agent mode.
