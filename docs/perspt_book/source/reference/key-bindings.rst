.. _reference-key-bindings:

Key Bindings
============

Keyboard shortcuts for Perspt TUI.

Chat Mode
---------

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Key
     - Action
   * - **Enter**
     - Send current message
   * - **Esc**
     - Exit application
   * - **Ctrl+C**
     - Force exit with cleanup
   * - **Ctrl+D**
     - Exit (EOF)
   * - **↑** / **↓**
     - Scroll chat history
   * - **Page Up** / **Page Down**
     - Fast scroll (10 lines)
   * - **Home** / **End**
     - Jump to top/bottom of history
   * - **Backspace**
     - Delete character before cursor
   * - **Delete**
     - Delete character at cursor
   * - **←** / **→**
     - Move cursor in input

Agent Mode (Dashboard)
----------------------

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Key
     - Action
   * - **q** / **Esc**
     - Exit application
   * - **Tab**
     - Switch between panels
   * - **↑** / **k**
     - Select previous item
   * - **↓** / **j**
     - Select next item
   * - **Enter**
     - Expand/view details
   * - **Space**
     - Toggle selection

Agent Mode (Review Modal)
-------------------------

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Key
     - Action
   * - **y**
     - Approve change
   * - **n**
     - Reject change
   * - **d**
     - View diff
   * - **e**
     - Edit before applying
   * - **Esc**
     - Cancel review
   * - **Enter**
     - Confirm action

Agent Mode (Diff Viewer)
------------------------

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Key
     - Action
   * - **↑** / **k**
     - Scroll up
   * - **↓** / **j**
     - Scroll down
   * - **Page Up** / **Page Down**
     - Fast scroll
   * - **g**
     - Jump to top
   * - **G**
     - Jump to bottom
   * - **q** / **Esc**
     - Close diff viewer

Commands
--------

Type these in the chat input:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Command
     - Action
   * - ``/save``
     - Save conversation (auto timestamp)
   * - ``/save <file>``
     - Save to specific file
   * - ``/clear``
     - Clear conversation history
   * - ``/help``
     - Show available commands
   * - ``/model <name>``
     - Switch model
   * - ``/quit``
     - Exit application

Vim-style Navigation
--------------------

For users who prefer vim bindings:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Key
     - Action
   * - **j**
     - Down
   * - **k**
     - Up
   * - **h**
     - Left (in input)
   * - **l**
     - Right (in input)
   * - **gg**
     - Top of history
   * - **G**
     - Bottom of history

See Also
--------

- :doc:`../tutorials/first-chat` - Getting started
- :doc:`cli-reference` - CLI commands
