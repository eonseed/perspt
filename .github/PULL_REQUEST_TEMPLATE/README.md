# Pull Request Templates

This directory contains organized pull request templates for different types of contributions to Perspt.

## Available Templates

### `general.md` (Default)
- **For**: Regular code changes, bug fixes, features, documentation updates
- **Use when**: Making standard contributions to the Perspt codebase
- **Auto-selected**: This template is automatically used for most pull requests

### `psp.md`
- **For**: Perspt Specification Proposal (PSP) submissions
- **Use when**: Submitting a new PSP document or updating existing PSPs
- **Manual selection**: Choose this template when creating PSP-related pull requests

## How to Use

### Automatic Template Selection
GitHub will automatically use the `general.md` template for most pull requests.

### Manual Template Selection
To use a specific template:

1. **Via URL Parameter**: Add `?template=<template-name>` to your PR URL
   - For PSP: `?template=psp.md`
   - For general: `?template=general.md`

2. **Via GitHub Interface**: When creating a PR, GitHub may show a template selection dropdown

## Template Guidelines

- **Fill out all relevant sections** in the chosen template
- **Mark checkboxes with an "x"** for completed items
- **Remove unused sections** if they don't apply to your change
- **Provide clear descriptions** to help reviewers understand your changes

## PSP-Specific Workflow

PSP pull requests trigger additional automation:
- 🔢 **Automatic PSP numbering** (if using XXXXXX placeholder)
- 💬 **Discussion issue creation** for community feedback
- 📋 **Status management** and validation
- 📚 **Documentation building** and deployment

For more details on the PSP process, see the [PSP Automation Guide](../PSP_AUTOMATION_GUIDE.md).
