.. _user-guide-providers:

Providers
=========

**Use an explicit model route when several providers are configured.** A route has the form ``provider::model``.
The provider prefix selects an adapter and the model part is passed through to it.
A bare model name uses the configured or detected provider.

Supported adapters
------------------

.. list-table::
   :header-rows: 1
   :widths: 25 25 50

   * - Provider
     - Prefix
     - Credential or connection
   * - OpenAI and compatible servers
     - ``openai``
     - ``OPENAI_API_KEY``; optional ``OPENAI_BASE_URL``.
   * - Anthropic
     - ``anthropic``
     - ``ANTHROPIC_API_KEY``.
   * - Google Gemini
     - ``gemini`` or ``google``
     - ``GEMINI_API_KEY``.
   * - Google Vertex AI
     - ``vertex``
     - ``VERTEX_PROJECT_ID`` and Application Default Credentials; optional ``VERTEX_LOCATION``.
   * - Groq
     - ``groq``
     - ``GROQ_API_KEY``.
   * - Cohere
     - ``cohere``
     - ``COHERE_API_KEY``.
   * - xAI
     - ``xai``
     - ``XAI_API_KEY``.
   * - DeepSeek
     - ``deepseek``
     - ``DEEPSEEK_API_KEY``.
   * - Ollama
     - ``ollama``
     - Local server; no API key. Optional ``OLLAMA_BASE_URL``.

Model access and tool-calling support depend on the account, server, and model.
Perspt's adapter list is not a catalog of available models.
``perspt providers`` shows configured routes; ``perspt providers --probe`` makes live requests to test their behavior.

Choose a route
--------------

Replace ``<model-id>`` with an identifier available through the selected provider:

.. code-block:: bash

   export OPENAI_API_KEY="<api-key>"
   perspt chat --model 'openai::<model-id>'

For local chat, select an installed model:

.. code-block:: bash

   ollama list
   perspt chat --model ollama::llama3.2

The local ID here is an example, not a requirement.
See :doc:`../tutorials/local-models` for setup.

Persist the default
-------------------

The configuration file can set a provider and model:

.. code-block:: toml

   provider = "openai"
   model = "<model-id>"

A CLI ``--model`` override takes precedence for that run.
Agent roles can use different fully qualified routes through ``[providers.<id>]`` and ``[models]``.
See :doc:`../howto/providers` for setup and :doc:`advanced-features` for role routing.

Vertex AI
---------

The Vertex adapter resolves ADC tokens per request unless ``VERTEX_API_KEY`` supplies an explicit bearer-token override.
``VERTEX_LOCATION`` defaults to ``global``.

.. code-block:: bash

   gcloud auth application-default login
   export VERTEX_PROJECT_ID="<project-id>"
   export VERTEX_LOCATION="global"
   perspt chat --model 'vertex::<model-id>'

OpenAI-compatible endpoints
---------------------------

A server must support the OpenAI request format expected by the adapter:

.. code-block:: bash

   export OPENAI_API_KEY="<server-key>"
   export OPENAI_BASE_URL="http://localhost:8000/v1"
   perspt chat --model 'openai::<served-model-id>'

A base URL override alone does not establish compatibility with a provider-specific authentication or deployment protocol.
