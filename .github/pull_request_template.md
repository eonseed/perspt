## Pull Request Type
<!-- ⚠️ IMPORTANT: Select the type of pull request. This determines which automation workflows will run. -->
- [ ] **General** - Standard code changes, bug fixes, features, documentation
- [ ] **PSP** - Perspt Specification Proposal (new PSP document or PSP updates)

---

## Description
<!-- Provide a brief description of the changes in this PR -->

## Type of Change
<!-- Mark the relevant option with an "x" -->
- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update
- [ ] Performance improvement
- [ ] Code cleanup/refactoring
- [ ] PSP document (new or updated Perspt Specification Proposal)

## Related Issues
<!-- Link to related issues using #issue_number -->
Fixes #(issue)

---

## PSP Information
<!-- ✅ FILL OUT ONLY IF YOU SELECTED "PSP" ABOVE -->
<!-- ❌ SKIP THIS SECTION FOR GENERAL PULL REQUESTS -->

### PSP Details
- **PSP Number**: <!-- e.g., 000042 (use 0000 if not assigned yet) -->
- **PSP Title**: <!-- e.g., Add Keyboard Shortcuts for File Operations -->
- **PSP Type**: <!-- Feature | Enhancement | UI/UX | API Change | Process -->
- **Related Proposal Issue**: <!-- Link to the initial PSP proposal issue if it exists -->

### PSP Status
- [ ] Draft (ready for initial review)
- [ ] Discussion (ready for community feedback)
- [ ] Revision (updating based on feedback)

### PSP Content Checklist
- [ ] **Preamble** with all required fields (PSP, Title, Author, Status, Type, Created, Discussion-To)
- [ ] **Abstract** - Brief 1-2 sentence summary
- [ ] **Motivation** - Clear problem statement and justification
- [ ] **Proposed Changes** - Detailed specification
- [ ] **Rationale** - Design decisions and alternatives considered
- [ ] **Backwards Compatibility** - Impact on existing users
- [ ] All sections are complete and well-written

---

## General Testing & Validation
<!-- Describe the tests you ran to verify your changes -->
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual testing completed
- [ ] PSP builds successfully with Sphinx (PSP PRs only)
- [ ] No formatting errors or warnings (PSP PRs only)

### Test Configuration
- **OS**: <!-- e.g., Ubuntu 22.04, macOS 13, Windows 11 -->
- **Rust Version**: <!-- e.g., 1.75.0 -->

## Implementation Details
<!-- For General PRs: Technical implementation notes -->
<!-- For PSP PRs: Implementation planning and feasibility -->
- [ ] Implementation plan is clear and feasible (PSP PRs)
- [ ] Breaking changes are clearly documented (if applicable)
- [ ] Migration path is provided (if needed)
- [ ] Performance implications are considered

## Additional Information

### For PSP PRs Only
- **Estimated Implementation Time**: <!-- e.g., 2-4 weeks -->
- **Dependencies**: <!-- Other PSPs, libraries, or requirements -->
- **Risks**: <!-- Potential challenges or concerns -->

### Screenshots (if applicable)
<!-- Add screenshots to help explain your changes, especially for UI/UX PSPs -->

## Final Checklist
<!-- Mark completed items with an "x" -->
- [ ] My code follows the project's style guidelines
- [ ] I have performed a self-review of my own code/document
- [ ] I have commented my code, particularly in hard-to-understand areas (General PRs)
- [ ] I have made corresponding changes to the documentation
- [ ] My changes generate no new warnings
- [ ] I have added tests that prove my fix is effective or that my feature works (General PRs)
- [ ] New and existing unit tests pass locally with my changes (General PRs)
- [ ] Any dependent changes have been merged and published
- [ ] PSP document follows the template structure (PSP PRs only)
- [ ] All RST formatting is correct (PSP PRs only)

## Additional Notes
<!-- Add any additional notes about the implementation or considerations for reviewers -->

---

<!-- 
🤖 AUTOMATION NOTES:
- PRs marked as "PSP" will trigger PSP automation workflows
- PSP PRs will auto-create discussion issues and handle numbering
- General PRs follow standard code review processes
-->
