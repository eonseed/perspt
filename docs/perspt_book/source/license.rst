License
=======

Perspt is released under the MIT License.

MIT License
-----------

Copyright (c) 2025 Ronak Rathoer, Vikrant Rathore

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

What This Means
---------------

The MIT License is one of the most permissive open source licenses. Here's what it means in practical terms:

✅ **What you CAN do:**

- **Use** Perspt for any purpose, including commercial projects
- **Modify** the source code to fit your needs
- **Distribute** copies of Perspt
- **Sublicense** and sell copies of your modifications
- **Use** Perspt in proprietary software
- **Include** Perspt in larger projects with different licenses

❗ **What you MUST do:**

- **Include** the copyright notice and license text in any copies or substantial portions
- **Preserve** the original license and copyright information

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
   * - **allms**
     - MIT/Apache-2.0
     - Unified LLM provider interface
   * - **reqwest**
     - MIT/Apache-2.0
     - HTTP client library
   * - **aws-sdk-bedrock**
     - Apache-2.0
     - AWS Bedrock SDK for Rust

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

The MIT License is compatible with most other open source licenses:

**Compatible Licenses:**
- Apache License 2.0
- BSD licenses (2-clause, 3-clause)
- ISC License
- Public Domain (CC0)
- LGPL (when used as a library)

**Special Considerations:**
- GPL v2/v3: Can use MIT-licensed code, but resulting work must be GPL
- Copyleft licenses: May require derivative works to use the same license

Commercial Use
--------------

Perspt can be freely used in commercial projects:

✅ **Allowed Commercial Uses:**

- **Internal tools** - Use Perspt as part of your development workflow
- **Embedded products** - Include Perspt in commercial software packages
- **Service offerings** - Provide Perspt as part of consulting or hosting services
- **Modified versions** - Create and sell modified versions of Perspt
- **Enterprise solutions** - Build enterprise tools based on Perspt

📋 **Requirements for Commercial Use:**

1. **Include license text** in your distribution
2. **Maintain copyright notices** from the original code
3. **No trademark usage** without permission (see below)

No additional fees, registrations, or permissions are required.

Trademark Policy
----------------

While the source code is MIT licensed, trademarks are handled separately:

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

1. **Your contributions** will be licensed under the same MIT License
2. **You have the right** to license your contributions under MIT
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
A: Yes, the MIT License allows this. Just include the license text.

**Q: Can I modify Perspt and sell the modified version?**
A: Yes, but you must include the original license and copyright notices.

**Q: Do I need to open source my modifications?**
A: No, the MIT License doesn't require you to share your changes.

**Q: Can I remove the copyright notices?**
A: No, you must preserve the copyright notices in all copies.

**Q: What if I only use parts of the code?**
A: The license still applies to any substantial portions you use.

**Q: Can I change the license of my derivative work?**
A: You can add additional licenses, but the MIT License must remain for the original parts.

**Q: Do I need to attribute Perspt in my application?**
A: While not legally required for end users, it's appreciated and good practice.

Getting Legal Advice
--------------------

This page provides general information about the MIT License and is not legal advice. For specific legal questions:

- **Consult** with a qualified attorney
- **Review** the full license text carefully
- **Consider** your specific use case and jurisdiction
- **Seek** professional legal counsel for commercial decisions

Reporting License Issues
------------------------

If you believe there's a license violation or have questions about licensing:

- **Email**: legal@perspt.dev
- **GitHub Issues**: `License Questions <https://github.com/yourusername/perspt/issues>`_
- **Include** specific details about the concern or question

We take licensing seriously and will investigate all reports promptly.

.. seealso::

   - :doc:`acknowledgments` - Credits and thanks to contributors
   - :doc:`developer-guide/contributing` - How to contribute to the project
   - `Open Source Initiative <https://opensource.org/licenses/MIT>`_ - Official MIT License text
