.. meta::
   :description: Perspt is an AI chat and coding assistant for the terminal. Chat with cloud or local models, ask an agent to edit code, and review build and test results.

Perspt
======

.. container:: hero

   .. container:: hero-copy

      .. container:: hero-main

         .. rst-class:: eyebrow

         PERSPT / AI TOOLS FOR DEVELOPERS

         .. raw:: html

            <h1>AI chat and coding<br>in your terminal.</h1>

         .. rst-class:: lead

         Ask questions, understand a codebase, or ask a coding agent to fix a bug. Perspt runs in the terminal, connects to cloud or local AI models, and uses the project's compiler and tests to check the agent's work.

         .. container:: actions

            `Get started <mount://book/quickstart.html>`_
            `Try the coding agent <mount://book/tutorials/agent-mode.html>`_

      .. container:: hero-footer

         .. rst-class:: hero-note

         Built in Rust. Works with OpenAI, Anthropic, Gemini, Ollama, and more.

         `GitHub <https://github.com/eonseed/perspt>`_ | `Read the book <mount://book/index.html>`_ | `Download PDF <mount://pdf/perspt-book-en.pdf>`_

   .. container:: hero-art

      .. raw:: html

         <div class="hero-terminal" data-mode="0" aria-label="Animated preview of Perspt's three terminal modes">
           <div class="term-bar">
             <div class="term-dots" aria-hidden="true">
               <span class="dot-red"></span>
               <span class="dot-amber"></span>
               <span class="dot-green"></span>
             </div>
             <div class="term-tabs" role="tablist" aria-label="Perspt modes">
               <button type="button" class="term-tab is-active" role="tab" aria-selected="true" data-mode-target="0">
                 <span class="tab-num">01</span> Simple CLI
               </button>
               <button type="button" class="term-tab" role="tab" aria-selected="false" data-mode-target="1">
                 <span class="tab-num">02</span> Terminal UI
               </button>
               <button type="button" class="term-tab" role="tab" aria-selected="false" data-mode-target="2">
                 <span class="tab-num">03</span> Coding Agent
               </button>
             </div>
           </div>
           <div class="term-progress" aria-hidden="true">
             <div class="term-progress-bar"></div>
           </div>
           <div class="term-viewport">
             <!-- Mode 1: Simple CLI (plain stdout streaming) -->
             <div class="term-stage stage-0 is-active" role="tabpanel" data-mode-panel="0">
               <div class="term-line step-1"><span class="t-muted">~/app</span> <span class="t-prompt">$</span> <span class="t-cmd t-typewriter">perspt simple-chat --model gemini-3.5-flash</span></div>
               <div class="term-line step-2 t-dim-line">Connected to <span class="t-cyan">gemini::gemini-3.5-flash</span> · plain stdout stream · Ctrl+C to exit</div>
               <div class="term-line step-3"><span class="t-user">You &gt;</span> Why does <code>"a,b,".split(',')</code> fail when parsing floats in Rust?</div>
               <div class="term-line step-4 t-stream-block">
                 <div><span class="t-mint">AI  &gt;</span> A trailing comma yields a final <code>""</code> slice, causing <code>parse::&lt;f64&gt;()</code> to fail.</div>
                 <div class="t-code-block">
                   <div><span class="t-kw">pub fn</span> <span class="t-fn">parse_row</span>(line: &amp;<span class="t-ty">str</span>) -&gt; Result&lt;Vec&lt;<span class="t-ty">f64</span>&gt;&gt; {</div>
                   <div>&nbsp;&nbsp;line.<span class="t-fn">split_terminator</span>(<span class="t-str">','</span>).<span class="t-fn">map</span>(str::parse).<span class="t-fn">collect</span>()</div>
                   <div>}</div>
                 </div>
               </div>
               <div class="term-line step-5"><span class="t-user">You &gt;</span> <span class="t-cmd">/save session.md</span> &nbsp;<span class="t-check">✓</span> <span class="t-muted">Saved 2 turns (240 ms)</span> <span class="t-cursor" aria-hidden="true">▋</span></div>
             </div>

             <!-- Mode 2: Full-Screen Ratatui TUI -->
             <div class="term-stage stage-1" role="tabpanel" data-mode-panel="1">
               <div class="term-line step-1 t-tui-topbar">
                 <span><strong class="t-mint">PERSPT CHAT</strong> · Full-Screen TUI</span>
                 <span class="t-cyan">anthropic::claude-sonnet-4-5</span>
                 <span class="t-gold">MCP: 2 servers</span>
               </div>
               <div class="term-line step-2 t-tui-pane">
                 <div class="t-pane-title">─ Conversation History ─────────────────────────────────────────</div>
                 <div class="t-chat-msg"><span class="t-badge-you">You</span> Summarize the 3 surfaces and show the gate inequality.</div>
                 <div class="t-chat-msg">
                   <span class="t-badge-ai">Assistant</span>
                   <div class="t-tui-table">
                     <div class="t-tbl-head"><span>Mode</span><span>Command</span><span>Workflow</span></div>
                     <div><span>Simple CLI</span><span><code>perspt simple-chat</code></span><span>Plain stdout &amp; shell logs</span></div>
                     <div><span>Terminal UI</span><span><code>perspt chat</code></span><span>Split-pane Markdown, math &amp; MCP</span></div>
                     <div><span>Coding Agent</span><span><code>perspt agent</code></span><span>Sandboxed edits + build/test gate</span></div>
                   </div>
                   <div class="t-math-box">Lyapunov descent gate: &nbsp;<span class="t-mint">V(x) ≤ V(x_best) − ρ_gate</span> &nbsp;<span class="t-muted">(hard pass: V(x) = 0)</span></div>
                 </div>
               </div>
               <div class="term-line step-3 t-tui-inputbox">
                 <div class="t-pane-title">─ Input (Enter: send · Ctrl+J: newline · Ctrl+R: reasoning) ────</div>
                 <div><span class="t-mint">&gt;</span> /mcp status <span class="t-cursor" aria-hidden="true">▋</span></div>
               </div>
             </div>

             <!-- Mode 3: Governed Coding Agent -->
             <div class="term-stage stage-2" role="tabpanel" data-mode-panel="2">
               <div class="term-line step-1"><span class="t-muted">~/app</span> <span class="t-prompt">$</span> <span class="t-cmd">perspt agent</span> <span class="t-flag">-w .</span> <span class="t-str">"Fix trailing-comma panic in CSV parser"</span></div>
               <div class="term-line step-2 t-agent-grid">
                 <div class="t-agent-box">
                   <div class="t-box-label">WORK GRAPH &amp; ENERGY V(x)</div>
                   <div class="t-energy-row"><span>V_syn</span><span class="t-bar-track"><span class="t-bar-fill fill-zero"></span></span><span class="t-mint">4.0 → 0.0</span></div>
                   <div class="t-energy-row"><span>V_log</span><span class="t-bar-track"><span class="t-bar-fill fill-zero"></span></span><span class="t-mint">3.0 → 0.0</span></div>
                   <div class="t-energy-row"><span>Total</span><span class="t-badge-pass">HardPass (V = 0.0)</span></div>
                 </div>
                 <div class="t-agent-box">
                   <div class="t-box-label">SANDBOXED VERIFIER SUITE</div>
                   <div class="t-vrow"><span class="t-check">✓</span> <span>syntax</span> <code>cargo check</code> <span class="t-mint">0 err</span></div>
                   <div class="t-vrow"><span class="t-check">✓</span> <span>build</span> <code>cargo build</code> <span class="t-mint">0.6s</span></div>
                   <div class="t-vrow"><span class="t-check">✓</span> <span>test</span> <code>cargo test</code> <span class="t-mint">18 ok</span></div>
                 </div>
               </div>
               <div class="term-line step-3 t-diff-box">
                 <div class="t-diff-head"><span>Review staged diff · <code>src/parser.rs</code></span><span class="t-muted">+1 −1</span></div>
                 <div class="t-diff-del">- &nbsp;line.split(',').map(str::parse).collect()</div>
                 <div class="t-diff-add">+ &nbsp;line.split_terminator(',').map(str::parse).collect()</div>
               </div>
               <div class="term-line step-4 t-review-bar">Apply to workspace? <span class="t-key">[y] Approve</span> <span class="t-key">[d] Diff</span> <span class="t-key">[c] Correct</span> <span class="t-key">[n] Reject</span> <span class="t-cursor" aria-hidden="true">▋</span></div>
             </div>
           </div>
           <div class="term-footer">
             <span class="term-footer-status">
               <span class="pulse-dot" aria-hidden="true"></span>
               <span class="term-mode-caption" data-caption-0="MODE 01/03 · PLAIN STREAMING CLI" data-caption-1="MODE 02/03 · FULL-SCREEN RATATUI TUI" data-caption-2="MODE 03/03 · VERIFIED CODING AGENT">MODE 01/03 · PLAIN STREAMING CLI</span>
             </span>
             <span class="term-footer-hint">AUTO-RUNNING · CLICK TAB TO SWITCH</span>
           </div>
         </div>

.. container:: section-heading

   .. rst-class:: eyebrow

   WHAT CAN PERSPT DO?

   .. rubric:: Chat with AI or put it to work on code.

.. container:: pillars

   .. container:: pillar

      .. rubric:: Chat with AI

      **Ask a question and read the answer as it arrives.** The full-screen chat displays Markdown, code, and tables. Paste a multiline question, switch models, or connect read-only MCP tools.

      .. code-block:: sh

         perspt chat

      `Start a conversation <mount://book/tutorials/first-chat.html>`_

   .. container:: pillar

      .. rubric:: Use a simpler chat interface

      **Type a message, get an answer, and keep the conversation in the terminal.** Use ``simple-chat`` for a line-by-line chat without a full-screen interface. Save the conversation with ``--log-file``.

      .. code-block:: sh

         perspt simple-chat --log-file chat.txt

      `Learn the chat commands <mount://book/user-guide/basic-usage.html>`_

   .. container:: pillar

      .. rubric:: Ask the agent to work on code

      **Fix a bug, add a feature, or write tests.** Give Perspt a task and a project folder. It edits a separate copy, runs the build and tests, and asks for review before applying the result.

      .. code-block:: sh

         perspt agent -w ./app "Fix the parser tests"

      `Follow the coding tutorial <mount://book/tutorials/agent-mode.html>`_

   .. container:: pillar

      .. rubric:: Use Perspt in a Rust application

      **Build an AI workflow with Perspt's libraries.** Reuse the code for running agents, checking their output, and recording session history. The SDK guide explains which crates to use.

      .. code-block:: sh

         cargo add perspt-sdk perspt-coding \
           --git https://github.com/eonseed/perspt

      `Read the SDK guide <mount://book/api/index.html>`_

.. container:: section-heading

   .. rst-class:: eyebrow

   HOW THE CODING AGENT WORKS

   .. rubric:: Describe the task. Review the code.

**Perspt works on a separate copy of the project.** The agent can read files, make edits, and run the project's tools. When a build or test fails, it uses the error messages to try a fix. The original files are updated after the required checks pass and the result is approved.

.. figure:: ../shared/_static/agent-workflow-overview.svg
   :target: _static/agent-workflow-overview.svg
   :alt: Give the agent a task, let it edit a project copy, run the build and tests, then review and apply the changes.
   :width: 100%

   The agent tries to fix build and test failures before asking for review. If it runs out of attempts, it stops and reports the remaining problems.

Agent mode is experimental. Passing tests can miss bugs, so review the diff and the tests before committing. Perspt does not create a Git commit when it applies edits.

.. container:: section-heading

   .. rst-class:: eyebrow

   LEARN FROM BUILD AND TEST FAILURES

   .. rubric:: Keep improvements. Reject regressions.

While required checks are failing, Perspt keeps an attempt only if its error score improves enough.
Otherwise, it restores the best version it has saved and uses the reported problems to try another fix.
An improvement can help the next attempt; the required checks still need to pass before the result can be applied.

.. figure:: ../shared/_static/front-page-abm.svg
   :target: _static/front-page-abm.svg
   :alt: An illustrative error score falls across kept attempts. Two attempts that make it worse are rejected. The final attempt passes the required checks.
   :width: 100%

   Dots show individual build and test results. The curves help us follow the attempts; this is an illustration, not benchmark data or a promise that every task will succeed.

.. container:: split-section

   .. container:: split-copy

      .. rst-class:: eyebrow

      FOLLOW THE WORK

      .. rubric:: See what the agent is doing.

      Add ``--dashboard`` to watch a run in the browser. See the task, build and test results, and the history of the agent's actions.

      ``perspt replay <session-id>`` reads the saved history without calling an AI model or rerunning tests. ``perspt resume --last`` continues an interrupted session.

      `Explore the dashboard <mount://book/user-guide/dashboard.html>`_

   .. container:: split-copy

      .. rst-class:: eyebrow

      STAY IN CONTROL

      .. rubric:: Review edits before they reach the project.

      Interactive runs ask for approval before applying code changes. Use ``--yes`` when an unattended run should apply a successful result automatically.

      ``perspt ledger --rollback <session-prefix>`` undoes a session's latest set of applied edits. It refuses to overwrite files changed afterward.

      Linux and macOS support OS sandboxing. Native Windows agent runs need an explicit option to run without that isolation, or WSL.

      `Read the agent guide <mount://book/user-guide/agent-mode.html>`_ | `Check setup requirements <mount://book/installation.html>`_

Install Perspt and start chatting
---------------------------------

With Rust 1.97.1 or later installed:

.. code-block:: sh

   git clone https://github.com/eonseed/perspt.git
   cd perspt
   cargo install --locked --path crates/perspt-cli --features bundled
   export GEMINI_API_KEY="<api-key>"
   perspt chat

For local AI models, connect Ollama and choose an installed model such as ``ollama::llama3.2``. To try the coding agent, start with a small task in a project whose build and tests already run locally.

.. container:: text-links

   `Quick start <mount://book/quickstart.html>`_
   `Local models <mount://book/tutorials/local-models.html>`_
   `Coding tutorial <mount://book/tutorials/agent-mode.html>`_
   `Rust SDK <mount://book/api/index.html>`_
   `How verification works <mount://book/concepts/srbn-architecture.html>`_
   `Specification proposals <mount://psps/index.html>`_
   `Rustdoc <perspt/index.html>`_
