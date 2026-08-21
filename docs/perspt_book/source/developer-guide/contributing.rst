.. _developer-guide-contributing:

Contributing
============

Development Setup
-----------------

.. code-block:: bash

   # Clone
   git clone https://github.com/eonseed/perspt.git
   cd perspt

   # Build
   cargo build

   # Run tests
   cargo test

   # Lint
   cargo clippy -- -D warnings

   # Format check
   cargo fmt -- --check


Project Structure
-----------------

.. code-block:: text

   crates/
     perspt-core/           # Types, config, LLM, events, plugins, prompts
     perspt-agent/          # Candidate runtime, tool loop, verifier, dispatch
     perspt-tui/            # Chat + Agent TUI
     perspt-cli/            # CLI entry (clap)
     perspt-store/          # DuckDB persistence
     perspt-policy/         # Starlark policies
     perspt-sandbox/        # Command sandboxing
     perspt-dashboard/      # Axum web dashboard
     perspt-sdk/            # Domain-neutral SRBN platform SDK
     perspt-prompt-macros/  # Build-time prompt section codegen
     perspt-coding/         # Coding domain package
     perspt-research/       # Research domain package
     perspt-benchmark/      # Optional credentialed evaluation harness
     perspt/                # Root integration crate
   xtask/                   # PSP code-rule checker (dev-only)
   tests/                   # Integration tests
   docs/                    # Sphinx documentation


Coding Standards
----------------

1. **Clippy clean** - ``cargo clippy -- -D warnings`` must pass
2. **Formatted** - ``cargo fmt`` with default settings
3. **Tests pass** - ``cargo test`` must pass all tests
4. **No ``println!`` in UI paths** - Use the event system or ``env_logger``
5. **PSP code rules** - Files <= 1408 lines, functions <= 70 code lines,
   lines <= 108 columns; enforced by ``./check-rules.sh check``
6. **Streaming safety** - Never block the UI select loop; spawn on tokio tasks

Commit Messages
---------------

- Describe what changed, not the sequence
- Do NOT include phase numbers or commit sequence numbers
- Keep the subject line under 72 characters

.. code-block:: text

   # Good
   Add sheaf validation for cross-language boundaries

   # Bad
   Commit 3/7: Phase 2 - Add sheaf validation


PR Workflow
-----------

1. Create a feature branch from ``master`` (or from the preceding feature
   branch for an explicitly stacked pull request).
2. Make changes with passing tests
3. Run the pull-request gate:

   .. code-block:: bash

      cargo fmt --all -- --check
      cargo clippy --locked --workspace --all-targets -- -D warnings
      cargo test --locked --workspace --all-targets -- --test-threads=1
      cargo check --locked -p perspt-cli --features benchmark
      cargo doc --locked --workspace --no-deps
      ./check-rules.sh check

   These commands use the exact DuckDB 1.5.5 shared library. CI downloads the
   official checksummed library matching the pinned Rust crate; local systems
   should install the same version. Release binaries alone use ``bundled``.
   The final ``cargo check`` covers the optional benchmark command without
   running any credentialed evaluation.

4. Push and open a PR
5. Address review feedback

CI Tiers
--------

The workflows separate quick review feedback from target-platform evidence:

* Every non-draft pull request runs one Ubuntu job containing format, Clippy,
  credential-free tests, optional-feature compilation, Rustdoc, and the PSP
  code rules. Stacked pull requests are supported because CI does not filter
  on the PR's base branch.
* A GitHub ``merge_group`` runs the same Ubuntu gate plus Windows and macOS
  tests against the temporary merge-queue commit. Enable a branch ruleset that
  requires the merge queue and the ``PR gate (Ubuntu)``, ``PSP code rules``,
  and ``Target OS`` checks to make this evidence a pre-merge requirement.
* Pushes to ``master`` or ``develop`` run the target-OS matrix as a safety net
  for direct merges. Maintainers can also use **Actions -> CI -> Run workflow**
  to request the Windows/macOS matrix before merging when a queue is not
  enabled. That manual target-only run does not repeat the Ubuntu quality gate.

The macOS job exercises its governed verifier backend. The native Windows job
tests chat/TUI and shared functionality, proves strict coding mode fails closed,
and runs a Rust verifier fixture only after explicit reduced-isolation opt-in.

Superseded PR runs are cancelled. Merge-queue and protected-branch runs are
never cancelled because their result is evidence for a specific commit.
PDF generation is release work rather than a pull-request gate.


Documentation
-------------

Documentation uses Sphinx with reStructuredText:

.. code-block:: bash

   # Build HTML docs
   cd docs/perspt_book && uv run make html

   # Build PSP docs
   cd docs/psps && uv run make html

   # Live preview
   cd docs/perspt_book && uv run sphinx-autobuild source build/html

CI treats Sphinx warnings as errors and disables remote intersphinx inventory
fetches so documentation validation is deterministic. One documentation
workflow publishes the Rust API, book, and PSPs as a single Pages artifact;
the release workflow owns optional PDF generation.
