.. _installation:

Installation
============

**Chat needs the Perspt binary and a model provider.** Coding tasks also need the project's own build and test tools.
The documentation publisher has no Python runtime dependency.

Build and install from source
-----------------------------

We need Rust 1.97.1 or later and a native C/C++ build toolchain for the bundled DuckDB build.
From the repository root:

.. code-block:: bash

   git clone https://github.com/eonseed/perspt.git
   cd perspt
   cargo install --locked --path crates/perspt-cli --features bundled
   perspt --version
   perspt --help

Cargo installs the executable on its binary path, usually ``~/.cargo/bin``.
The ``bundled`` feature compiles DuckDB into the executable.
Compilation can take several minutes.

To build without installing:

.. code-block:: bash

   cargo build --locked -p perspt-cli --release --features bundled
   ./target/release/perspt --version

The workspace root is a virtual manifest, so local installation targets ``crates/perspt-cli``.
The ``perspt`` crate is a library facade; the executable belongs to ``perspt-cli``.

For development builds without ``bundled``, we need the exact DuckDB shared library matching ``Cargo.toml``.
The CI setup in ``.github/actions/setup-duckdb/action.yml`` pins the library and its checksums.
See :doc:`developer-guide/contributing` for development checks.

Connect a model
---------------

Set one credential and start chat:

.. code-block:: bash

   export GEMINI_API_KEY="<api-key>"
   perspt chat

Other supported providers and explicit ``provider::model`` routes are described in :doc:`user-guide/providers`.
A modern terminal with UTF-8 and color support gives the best full-screen experience.
``perspt simple-chat`` provides line-oriented terminal chat.

For local execution, start ``ollama serve`` in another terminal, pull a model, and select it explicitly:

.. code-block:: bash

   ollama pull llama3.2
   perspt chat --model ollama::llama3.2

Prepare a coding project
------------------------

**Run the project's checks locally before starting an agent task.** Verification processes run without network access under the normal sandbox policy.
We install or sync dependencies first, so checks can run offline.
``--allow-dependency-mutation`` grants agent dependency operations when needed; it does not bypass other policies.

.. list-table::
   :header-rows: 1
   :widths: 22 78

   * - Project
     - Tools and preparation
   * - **Rust**
     - Install Cargo and rustc. Add ``rust-analyzer`` with ``rustup component add rust-analyzer``. Fetch dependencies and confirm ``cargo check`` and ``cargo test`` work.
   * - **Python**
     - Install Python and uv. Sync the project's environment, including pytest. The syntax stage uses host ``ty`` when available, otherwise offline ``compileall`` through uv; Pyright is a separately detected fallback. The LSP prefers ``ty`` with ``pyright-langserver`` as fallback.
   * - **JavaScript / TypeScript**
     - Install Node.js, npm, TypeScript, and ``typescript-language-server``. Install project dependencies and configure the build and test scripts. The plugin runs ``npx tsc --noEmit``, ``npm run build``, and ``npm test``.

The built-in plugins cover Rust, Python, and JavaScript/TypeScript.
There is no built-in Go plugin.
Python project tools are optional for working on Python projects and benchmark fixtures; they are not needed to publish these docs.

Process isolation
-----------------

Linux agent runs use Bubblewrap, which must be installed and usable on the host.
macOS uses ``sandbox-exec``.
A missing required isolation backend blocks process execution by default.

Chat and plain terminal chat work natively on Windows.
Native Windows has no Perspt OS process-sandbox backend.
WSL provides the Linux isolation path.
To explicitly permit host-user execution on native Windows:

.. code-block:: powershell

   perspt agent --allow-unisolated -w . "Fix the parser tests"

For resumed runs, the durable setting is:

.. code-block:: toml

   [verification]
   allow_unisolated = true

The agent still works on a project copy, limits permitted edits, runs required checks, and asks for approval.
Child processes in reduced isolation have the host user's filesystem and network access.

Build the documentation
-----------------------

Install Guidedog and confirm ``guidedog --version`` succeeds.
Guidedog includes its PDF and Graphviz engines.
The standalone Typst CLI is needed when changing the diagram and cover sources; builds can otherwise use the checked-in assets.

.. code-block:: bash

   # Complete website, book PDF, and proposal PDFs
   cargo run -p xtask -- docs

   # HTML only
   cargo run -p xtask -- docs --no-pdf

   # Regenerate figures and cover artwork after source edits
   cargo run -p xtask -- assets

   # Live preview
   cargo run -p xtask -- serve

The site is written to ``docs/_build/site/index.html``.
The book PDF is written to ``docs/_build/site/books/perspt-book-en.pdf`` and copied to ``docs/perspt.pdf``.
See :doc:`developer-guide/documentation` for the publication checks.
