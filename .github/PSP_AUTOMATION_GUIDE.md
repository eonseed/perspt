# PSP GitHub Automation Guide

This document explains how Perspt Specification Proposals (PSPs) are authored, validated, and published using **Guidedog (`guidedog dsc`)** and GitHub Actions.

## Quick Start for PSP Authors

1. Create a new PSP record using Guidedog:
   ```bash
   guidedog dsc new "Your PSP Title" --dir=docs/psps
   ```
2. Edit the generated file in `docs/psps/records/XXXXX-your-psp-title.rst`.
3. Refresh the registry and validate all metadata and cross-references:
   ```bash
   guidedog dsc index --dir=docs/psps
   guidedog dsc check --dir=docs/psps
   guidedog dsc build --dir=docs/psps
   ```
4. Open a pull request.

## Workflows

### 1. PSP Automation (`.github/workflows/psp-automation.yml`)
- Validates modified records in `docs/psps/records/*.rst` using `guidedog dsc check --dir=docs/psps`, `guidedog dsc build --dir=docs/psps`, and `guidedog build html docs/psps -W -n`.
- Posts a summary comment on new PSP pull requests.
- Validates both unnumbered drafts and numbered records. It never numbers proposals or changes their state on merge.

Maintainers use `guidedog dsc promote <slug> --dir=docs/psps` to assign a number while keeping the proposal in `draft`. Opening discussion is a separate `guidedog dsc state <number> --to=discussion --reason="Ready for review" --dir=docs/psps` decision. These edits go through reviewed pull requests.

### 2. Documentation & GitHub Pages (`.github/workflows/docs.yml`)
- Installs the checksum-pinned `guidedog` publisher (with embedded Typst PDF and Graphviz support).
- Builds the PSP catalog (HTML + individual Typst PDFs), the Perspt Book (HTML + Typst PDF), the unified landing site (`guidedog ssg build docs --out docs/_build/site`), and Rustdoc (`cargo doc`), and deploys the unified site to GitHub Pages.

### 3. PSP Review Reminders (`.github/workflows/psp-status-management.yml`)
- Reports discussion records that have not changed for 60 days. It has read-only repository access and never changes proposal metadata.

The custom lifecycle is defined in `docs/psps/discussion.toml`: `unassigned` authoring, numbered `draft`, `discussion`, and maintainer decisions such as `accepted`, `active`, `final`, `rejected`, or `withdrawn`.

## Related Documentation

- [PSP Catalog README](../docs/psps/README.rst)
- [PSP-000001: PSP Purpose and Guidelines](../docs/psps/records/000001-psp-purpose-and-guidelines.rst)
- [PSP Maintainer Guide](../docs/psps/MAINTAINER_GUIDE.rst)
