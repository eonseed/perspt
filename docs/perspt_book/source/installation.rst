.. _installation:

Installation
============

To deploy and execute Perspt, the environment must satisfy the specified system requirements. This document outlines the procedures for building the application, configuring provider keys, and setting up domain-specific tools.

System Requirements
-------------------

- **Operating System**: macOS, Linux, or Windows (via Windows Subsystem for Linux).
- **Rust Toolchain**: Version 1.82.0 or later (required for compilation from source).
- **Terminal Emulator**: Must support 256-color escape codes and UTF-8 encoding.

Building from Source
--------------------

To compile the executable from the source repository:

.. code-block:: bash

   # Clone the source repository
   git clone https://github.com/eonseed/perspt.git
   cd perspt

   # Compile the project in release mode
   cargo build --release

The compiled binary will be placed at ``target/release/perspt``. To make the command globally accessible, copy the binary to a directory in your system ``PATH``, for example:

.. code-block:: bash

   cp target/release/perspt ~/.local/bin/

Installing via Cargo
--------------------

If you possess the Rust toolchain and wish to install directly from the local directory:

.. code-block:: bash

   cargo install --path .

Verifying the Installation
--------------------------

To verify that the executable is operational, query the system version and help manual:

.. code-block:: bash

   perspt --version
   # Expected output: perspt 0.6.1

   perspt --help

Configuring API Keys
--------------------

The application requires access to an LLM provider. Set the appropriate API key as an environment variable. The program will automatically detect the provider from the environment:

.. code-block:: bash

   # For Google Gemini
   export GEMINI_API_KEY="your-api-key"

   # For OpenAI
   export OPENAI_API_KEY="sk-..."

   # For Anthropic
   export ANTHROPIC_API_KEY="sk-ant-..."

If you are using Ollama for local execution, start the local service. No API key is required:

.. code-block:: bash

   ollama serve
   ollama pull llama3.2

Agent Mode Prerequisites
------------------------

Autonomous agent execution requires the presence of domain-specific compilers, type checkers, and test runners on the system path.

Python Domain
~~~~~~~~~~~~~

The Python adapter requires:

1. **uv**: For dependency management and environment isolation.
2. **ty**: For static type verification.
3. **pytest**: For logical assertion testing.

Install these dependencies as follows:

.. code-block:: bash

   # Install the uv package manager
   curl -LsSf https://astral.sh/uv/install.sh | sh

   # Install verification tools
   pip install ty pytest

Rust Domain
~~~~~~~~~~~

The Rust adapter requires:

1. **rustup**: For compiler toolchain management.
2. **rust-analyzer**: For language server diagnostics.

Install these components as follows:

.. code-block:: bash

   # Install the Rust compiler and manager
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # Install the LSP component
   rustup component add rust-analyzer

JavaScript/TypeScript Domain
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The TypeScript adapter requires Node.js and the TypeScript compiler:

.. code-block:: bash

   # Install Node.js via your system package manager, then install the compiler
   npm install -g typescript

Building the Documentation
--------------------------

To compile the Sphinx-based documentation books locally:

.. code-block:: bash

   # Ensure the uv tool is installed, then build the HTML output
   cd docs/perspt_book
   uv run make html

   # To compile to a PDF file (requires a LaTeX distribution like TeX Live)
   uv run make latexpdf

The generated HTML files are located at ``docs/perspt_book/build/html/index.html``, and the PDF is placed at ``docs/perspt_book/build/latex/perspt.pdf``.
