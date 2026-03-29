.. _example-scientific:

Example: Scientific Computing
=============================

This tutorial demonstrates using Perspt's headless agent mode for scientific
computing tasks: CFD simulation setup and wind tunnel data analysis.

.. admonition:: Experimental
   :class: warning

   These examples demonstrate advanced prompt patterns for scientific computing.
   Results depend on model capability and may require iteration. Treat them as
   starting points rather than guaranteed production workflows.


CFD Simulation Setup (Python)
-----------------------------

Create a computational fluid dynamics simulation scaffolding:

.. code-block:: bash

   export GEMINI_API_KEY="your-key"

   perspt agent --yes --defer-tests -w /tmp/cfd-sim \
     --architect-model gemini-pro-latest \
     --actuator-model gemini-3.1-flash-lite-preview \
     "Create a Python CFD simulation package that:
      1. Defines a 2D grid mesh using numpy arrays
      2. Implements a simple Navier-Stokes solver (lid-driven cavity)
      3. Uses finite difference method for spatial discretization
      4. Includes a Jacobi iterative pressure solver
      5. Outputs velocity and pressure fields as numpy arrays
      6. Includes visualization with matplotlib (contour plots)
      7. Has pytest tests validating conservation laws
      8. Uses Pydantic for simulation parameters (Re, dt, grid_size)"

Expected output structure:

.. code-block:: text

   /tmp/cfd-sim/
   +-- pyproject.toml
   +-- src/
   |   +-- cfd_sim/
   |   |   +-- __init__.py
   |   |   +-- mesh.py          # Grid generation
   |   |   +-- solver.py        # Navier-Stokes solver
   |   |   +-- pressure.py      # Jacobi pressure solver
   |   |   +-- visualization.py # matplotlib plotting
   |   |   +-- parameters.py    # Pydantic config
   |   +-- main.py
   +-- tests/
       +-- test_mesh.py
       +-- test_solver.py


Wind Tunnel Data Analysis (Python)
-----------------------------------

Analyze experimental wind tunnel data:

.. code-block:: bash

   perspt agent --yes --defer-tests -w /tmp/wind-tunnel \
     --architect-model gemini-pro-latest \
     --actuator-model gemini-3.1-flash-lite-preview \
     "Create a Python wind tunnel data analysis package that:
      1. Reads pressure tap data from CSV files
      2. Computes lift and drag coefficients (Cl, Cd) from pressure distributions
      3. Implements trapezoidal integration for force computation
      4. Generates polar plots of pressure coefficient (Cp) distribution
      5. Supports multiple angle-of-attack sweeps
      6. Includes Pydantic models for AirfoilData and TestConditions
      7. Exports results as JSON and matplotlib figures
      8. Has pytest tests with known NACA 0012 reference data"


Finite Element Solver (Rust)
-----------------------------

Build a simple FEM solver in Rust:

.. code-block:: bash

   perspt agent --yes -w /tmp/fem-solver \
     --architect-model gemini-pro-latest \
     --actuator-model gemini-3.1-flash-lite-preview \
     "Create a Rust finite element package that:
      1. Defines 2D triangular mesh elements
      2. Assembles global stiffness matrix for heat conduction
      3. Applies Dirichlet boundary conditions
      4. Solves using Gauss elimination (nalgebra)
      5. Outputs nodal temperatures
      6. Includes tests with known analytical solutions"


Tips for Scientific Tasks
--------------------------

1. **Be specific about algorithms** — Name the numerical method, discretization
   scheme, and solver type in the prompt
2. **Include validation criteria** — Reference known analytical solutions or
   benchmark data for tests
3. **Use --defer-tests** — Scientific tests often depend on numerical tolerances
   that need tuning after initial generation
4. **Set higher cost limits** — Scientific code often requires more iterations
   for convergence: ``--max-cost 10.0``
5. **Review carefully** — Numerical correctness requires domain expertise beyond
   what the LSP can verify

See Also
--------

- :doc:`headless-mode` — Headless mode guide
- :doc:`agent-mode` — Interactive agent mode
