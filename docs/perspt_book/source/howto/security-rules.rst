.. _howto-security-rules:

Security Rules
==============

How to configure Starlark policy rules for agent operations.

Overview
--------

The policy engine evaluates commands before execution using Starlark rules.
This prevents dangerous operations and provides fine-grained control.

Initialize Rules
----------------

.. code-block:: bash

   perspt init --rules

This creates ``.perspt/rules.star``:

.. code-block:: text

   my-project/
   └── .perspt/
       └── rules.star

Rule Syntax
-----------

Rules use three functions:

.. code-block:: python

   allow("pattern")                    # Always allow
   prompt("pattern", reason="...")     # Ask user
   deny("pattern", reason="...")       # Always deny

**Patterns** support glob syntax:

- ``*`` — Any characters
- ``?`` — Single character
- ``[abc]`` — Character set

Default Rules
-------------

.. code-block:: python

   # .perspt/rules.star

   # Safe read operations
   allow("cat *")
   allow("head *")
   allow("tail *")
   allow("ls *")
   allow("find *")
   allow("grep *")

   # Prompt for modifications
   prompt("rm *", reason="File deletion")
   prompt("mv *", reason="File move/rename")
   prompt("cp *", reason="File copy")
   prompt("chmod *", reason="Permission change")

   # Deny dangerous operations
   deny("rm -rf /", reason="Root deletion")
   deny("rm -rf ~", reason="Home deletion")
   deny("rm -rf /*", reason="Wildcard root deletion")
   deny("chmod 777 *", reason="Insecure permissions")
   deny("curl * | bash", reason="Remote code execution")
   deny("wget * | bash", reason="Remote code execution")

Custom Rules
------------

For a Node.js project:

.. code-block:: python

   # .perspt/rules.star

   # Allow package management
   allow("npm install")
   allow("npm test")
   allow("npm run *")

   # Prompt for global operations
   prompt("npm install -g *", reason="Global install")
   prompt("npm uninstall *", reason="Package removal")

   # Deny destructive
   deny("rm -rf node_modules", reason="Use npm prune instead")

For a Python project:

.. code-block:: python

   # .perspt/rules.star

   # Allow common operations
   allow("python *")
   allow("pip install *")
   allow("pytest *")
   allow("uv *")

   # Prompt for system changes
   prompt("pip uninstall *", reason="Package removal")
   prompt("pip install --user *", reason="User install")

   # Deny dangerous
   deny("pip install --break-system-packages *")

Testing Rules
-------------

Test your rules before deployment:

.. code-block:: bash

   # Simulate a command
   perspt policy test "rm -rf /"
   # Output: DENIED - Root deletion

   perspt policy test "cat README.md"
   # Output: ALLOWED

Project-Specific Overrides
--------------------------

Rules are inherited hierarchically:

1. Global: ``~/.perspt/rules.star``
2. Project: ``.perspt/rules.star``

Project rules override global rules.

Bypass for Trusted Tasks
------------------------

Use ``-y`` or ``--mode yolo`` to skip policy checks (⚠️ dangerous):

.. code-block:: bash

   perspt agent -y "Install dependencies"

See Also
--------

- :doc:`../api/perspt-policy` - Policy engine API
- :doc:`agent-options` - Agent configuration
