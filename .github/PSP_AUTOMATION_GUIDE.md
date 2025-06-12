# PSP GitHub Automation Guide

This document explains the GitHub automation workflows for managing Perspt Specification Proposals (PSPs).

## 🎯 Overview

The PSP automation system provides:
- **Automatic PSP numbering** for new proposals
- **Discussion issue creation** for community feedback
- **Status management** throughout the PSP lifecycle
- **Documentation building** and validation
- **Stale PSP detection** and maintenance

## 🔧 Workflows

### 1. PSP Automation (`psp-automation.yml`)

**Triggers:** PSP file changes in PRs, PSP proposal issues
**Functions:**
- Detects PSP-related pull requests
- Assigns PSP numbers automatically (replaces XXXXXX placeholders)
- Creates discussion issues for community feedback
- Updates PSP status to "Discussion" when merged
- Adds helpful comments to PSP proposal issues

### 2. PSP Documentation (`psp-docs.yml`)

**Triggers:** Changes to PSP documentation, manual dispatch
**Functions:**
- Builds PSP documentation with Sphinx
- Generates workflow diagrams from .dot files
- Validates PSP document format and content
- Checks for duplicate PSP numbers
- Uploads documentation artifacts

### 3. PSP Status Management (`psp-status-management.yml`)

**Triggers:** Manual dispatch, issue closures, scheduled (weekly)
**Functions:**
- Manual PSP status updates by maintainers
- Automatic status updates based on discussion issue outcomes
- Detects stale PSPs (>60 days in Discussion)
- Creates maintenance issues for stale PSPs

## 📝 Issue Templates

### PSP Proposal (`psp_proposal.yml`)
- For initial PSP ideas and community discussion
- Captures all essential information before formal PSP creation
- Auto-assigns labels and provides guidance

### PSP Discussion (`psp_discussion.yml`)
- For discussing specific PSP documents
- Auto-created by workflows when PSP PRs are submitted
- Links to the PSP document and provides discussion guidelines

## 🔄 PSP Lifecycle Automation

### 1. Initial Proposal
```
User creates PSP proposal issue → Auto-comment with guidance
```

### 2. PSP Document Creation
```
User creates PR with PSP file (using XXXXXX) → Workflow detects PSP
→ Assigns next available PSP number → Creates discussion issue
→ Adds comment linking PR to discussion
```

### 3. Community Discussion
```
Community discusses in auto-created issue → Feedback collected
```

### 4. Decision Making
```
Maintainer adds label (psp-accepted/psp-rejected/psp-withdrawn)
→ Closes discussion issue → Workflow updates PSP status automatically
```

### 5. Implementation (if accepted)
```
Implementation PRs reference PSP number → Manual status update to "Final"
```

## 🛠️ Maintainer Actions

### Assigning PSP Numbers
**Automatic:** Workflows detect XXXXXX placeholders and assign next available number
**Manual:** If needed, maintainers can edit PSP files directly

### Updating PSP Status
1. **Via Workflow Dispatch:**
   - Go to Actions → PSP Status Management
   - Run workflow with PSP number and new status
   
2. **Via Discussion Issue Labels:**
   - Add `psp-accepted`, `psp-rejected`, or `psp-withdrawn` label
   - Close the discussion issue
   - Workflow will update PSP status automatically

### Handling Stale PSPs
- Weekly check identifies PSPs in Discussion >60 days
- Creates maintenance issue with list of stale PSPs
- Review each PSP and make decisions
- Use status update workflow to change status

## 📋 PSP File Requirements

### Filename Format
- New PSPs: `psp-XXXXXX-title.rst` (placeholder)
- Assigned: `psp-000042-title.rst` (6-digit number)

### Required Fields
```rst
PSP: 000042
Title: Your PSP Title
Author: Your Name (@github_username)
Status: Draft
Type: Feature|Enhancement|UI/UX|API Change|Process
Created: YYYY-MM-DD
Discussion-To: (auto-filled by workflow)
```

### Status Flow
```
Draft → Discussion → Accepted/Rejected/Withdrawn → Final (if implemented)
```

## 🔍 Troubleshooting

### PSP Number Not Assigned
- Check that filename uses XXXXXX placeholder
- Ensure PR modifies files in `docs/psps/source/`
- Workflow only processes files matching `psp-[0-9]{6}*.rst` pattern

### Discussion Issue Not Created
- Verify PSP metadata is properly formatted
- Check that PR contains valid PSP file changes
- Review workflow run logs for errors

### Status Not Updating
- Ensure discussion issue has correct labels
- Verify PSP file exists and is properly formatted
- Check that maintainer has proper permissions

## 📚 Related Documentation

- [PSP Process Guidelines](../docs/psps/source/psp-000001.rst)
- [PSP Index](../docs/psps/source/psp-000000.rst)
- [PSP Template](../docs/psps/source/psp-template.rst)
- [Maintainer Guide](../docs/psps/MAINTAINER_GUIDE.rst)

## 🆘 Getting Help

If automation isn't working as expected:
1. Check the workflow run logs in GitHub Actions
2. Verify file naming and format requirements
3. Review PSP document metadata
4. Contact maintainers for manual intervention if needed

The automation is designed to be helpful but not block the PSP process - maintainers can always intervene manually when needed.
