License
=======

Perspt is released under the GNU Lesser General Public License v3.0 (LGPL v3).

LGPL v3 License
---------------

Copyright (c) 2025 Ronak Rathoer, Vikrant Rathore

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Lesser General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU Lesser General Public License for more details.

You should have received a copy of the GNU Lesser General Public License
along with this program. If not, see <https://www.gnu.org/licenses/>.

What This Means
---------------

The LGPL v3 is a copyleft license that provides strong protection for software freedom while allowing linking with proprietary software. Here's what it means in practical terms:

✅ **What you CAN do:**

- **Use** Perspt for any purpose, including commercial projects
- **Modify** the source code to fit your needs
- **Distribute** copies of Perspt
- **Link** Perspt as a library in proprietary software
- **Combine** Perspt with software under different licenses
- **Create** derivative works based on Perspt

❗ **What you MUST do:**

- **Provide source code** for any modifications to Perspt itself
- **Include** the LGPL v3 license text with distributions
- **Preserve** copyright notices and license information
- **Allow users** to replace the Perspt library with modified versions
- **Make modified source** available under LGPL v3 terms

🚫 **What we DON'T provide:**

- **Warranty** - The software is provided "as is"
- **Liability coverage** - We're not responsible for any damages
- **Support guarantees** - While we strive to help, support is provided on a best-effort basis

Third-Party Licenses
--------------------

Perspt depends on several open source libraries, each with their own licenses:

Core Dependencies
~~~~~~~~~~~~~~~~~

.. list-table::
   :widths: 25 25 50
   :header-rows: 1

   * - Crate
     - License
     - Description
   * - **tokio**
     - MIT
     - Async runtime for Rust
   * - **ratatui**
     - MIT
     - Terminal user interface library
   * - **serde**
     - MIT/Apache-2.0
     - Serialization framework
   * - **clap**
     - MIT/Apache-2.0
     - Command line argument parser
   * - **anyhow**
     - MIT/Apache-2.0
     - Error handling library
   * - **thiserror**
     - MIT/Apache-2.0
     - Error derive macros

LLM Integration
~~~~~~~~~~~~~~~

.. list-table::
   :widths: 25 25 50
   :header-rows: 1

   * - Crate
     - License
     - Description
   * - **genai**
     - MIT/Apache-2.0
     - Unified LLM provider interface
   * - **reqwest**
     - MIT/Apache-2.0
     - HTTP client library

Terminal and UI
~~~~~~~~~~~~~~~

.. list-table::
   :widths: 25 25 50
   :header-rows: 1

   * - Crate
     - License
     - Description
   * - **crossterm**
     - MIT
     - Cross-platform terminal library
   * - **unicode-width**
     - MIT/Apache-2.0
     - Unicode character width calculation
   * - **textwrap**
     - MIT
     - Text wrapping and formatting

Development Dependencies
~~~~~~~~~~~~~~~~~~~~~~~~

.. list-table::
   :widths: 25 25 50
   :header-rows: 1

   * - Crate
     - License
     - Description
   * - **criterion**
     - MIT/Apache-2.0
     - Benchmarking library
   * - **mockall**
     - MIT/Apache-2.0
     - Mock object library
   * - **tempfile**
     - MIT/Apache-2.0
     - Temporary file management

License Compatibility
---------------------

The LGPL v3 is compatible with most other open source licenses:

**Compatible Licenses:**
- Apache License 2.0
- BSD licenses (2-clause, 3-clause)
- ISC License
- MIT License
- Public Domain (CC0)
- GPL v3+ (can be upgraded to GPL)

**Special Considerations:**
- GPL v2: Not directly compatible due to version differences
- Proprietary licenses: Can link with LGPL libraries but must allow library replacement
- Copyleft licenses: LGPL provides weaker copyleft than GPL

Commercial Use
--------------

Perspt can be freely used in commercial projects:

✅ **Allowed Commercial Uses:**

- **Internal tools** - Use Perspt as part of your development workflow
- **Linked libraries** - Link Perspt as a library in commercial software
- **Service offerings** - Provide Perspt as part of consulting or hosting services
- **Modified library versions** - Create modified versions for internal use
- **Enterprise solutions** - Build enterprise tools that use Perspt

📋 **Requirements for Commercial Use:**

1. **Include LGPL license text** in your distribution
2. **Maintain copyright notices** from the original code
3. **Provide source code** for any modifications to Perspt itself
4. **Allow library replacement** - users must be able to replace the Perspt library
5. **No trademark usage** without permission (see below)

No additional fees, registrations, or permissions are required.

Trademark Policy
----------------

While the source code is LGPL v3 licensed, trademarks are handled separately:

**"Perspt" Name and Logo:**
- The name "Perspt" and any associated logos are trademarks
- You may use the name in accurately describing the software
- Commercial use of the name/logo as your own brand requires permission
- Modified versions should use different names to avoid confusion

**Acceptable Uses:**
- "Built with Perspt"
- "Based on Perspt"
- "Powered by Perspt"
- "Fork of Perspt"

**Requires Permission:**
- Using "Perspt" as your product name
- Using Perspt logos in your branding
- Implying official endorsement

Contributing and License
------------------------

By contributing to Perspt, you agree that:

1. **Your contributions** will be licensed under the same LGPL v3 License
2. **You have the right** to license your contributions under LGPL v3
3. **You understand** that your contributions may be used commercially
4. **You retain copyright** to your contributions while granting broad usage rights

Contributor License Agreement (CLA)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

For substantial contributions, we may request a Contributor License Agreement to:

- Ensure you have the right to contribute the code
- Provide legal protection for the project and users
- Allow for potential future license changes if needed
- Clarify the rights and responsibilities of contributors

License FAQ
-----------

**Q: Can I use Perspt in my proprietary software?**
A: Yes, LGPL v3 allows linking with proprietary software. You must provide the library source and allow replacement.

**Q: Can I modify Perspt and sell the modified version?**
A: Yes, but you must provide the source code for your modifications under LGPL v3.

**Q: Do I need to open source my modifications?**
A: Yes, any modifications to Perspt itself must be made available under LGPL v3.

**Q: Can I remove the copyright notices?**
A: No, you must preserve the copyright notices and license information in all copies.

**Q: What if I only use parts of the code?**
A: The LGPL v3 license still applies to any substantial portions you use.

**Q: Can I change the license of my derivative work?**
A: You can license your own code separately, but Perspt parts must remain LGPL v3.

**Q: Do I need to attribute Perspt in my application?**
A: Yes, you must include the LGPL v3 license and copyright notices.

Getting Legal Advice
--------------------

This page provides general information about the LGPL v3 License and is not legal advice. For specific legal questions:

- **Consult** with a qualified attorney
- **Review** the full license text carefully
- **Consider** your specific use case and jurisdiction
- **Seek** professional legal counsel for commercial decisions

Reporting License Issues
------------------------

If you believe there's a license violation or have questions about licensing:

- **Email**: legal@perspt.dev
- **GitHub Issues**: `License Questions <https://github.com/eonseed/perspt/issues>`_
- **Include** specific details about the concern or question

We take licensing seriously and will investigate all reports promptly.

.. seealso::

   - :doc:`acknowledgments` - Credits and thanks to contributors
   - :doc:`developer-guide/contributing` - How to contribute to the project
   - `GNU Project <https://www.gnu.org/licenses/lgpl-3.0.html>`_ - Official LGPL v3 License text
