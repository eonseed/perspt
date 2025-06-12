# PSP GitHub Automation Guide

This document explains the GitHub automation workflows for managing Perspt Specification Proposals (PSPs).

## 🚀 Quick Start

**For PSP Authors:**
1. Create a PR with your Draft PSP file named `psp-0000-your-title.rst`
2. Use `PSP: 0000` in the file header 
3. Set `Status: Draft`
4. Submit PR for maintainer review

**What happens automatically after merge:**
- PSP gets assigned the next available number
- Status changes from "Draft" to "Discussion"
- Discussion issue is created for community feedback
- PSP file is updated with the Discussion-To link

**No additional PRs needed** - the automation handles all transitions!

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
- Assigns PSP numbers automatically (replaces 0000 placeholders)
- Automatically transitions Draft PSPs to Discussion status upon merge
- Creates discussion issues for community feedback
- Updates PSP files with Discussion-To links
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

### 1. Initial Proposal (Optional)
```
User creates PSP proposal issue → Auto-comment with guidance
```

### 2. PSP Document Creation
```
User creates PR with Draft PSP file (using 0000 placeholder)
→ Maintainer reviews and merges PR
→ Workflow assigns next available PSP number
→ Workflow automatically updates status to "Discussion"  
→ Workflow creates discussion issue
→ Workflow updates PSP file with Discussion-To link
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
- New PSPs: `psp-0000-title.rst` (using 0000 placeholder)
- Assigned: `psp-000042-title.rst` (6-digit number)

### Required Fields
```rst
PSP: 0000
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

## � Complete PSP Process Flow

The PSP automation provides a streamlined process:

1. **Author submits PR** with Draft PSP file (using `psp-0000-title.rst`)
2. **Maintainer reviews and merges** the PR
3. **Automation automatically:**
   - Assigns next available PSP number
   - Updates filename and content
   - Changes status from "Draft" to "Discussion"
   - Creates discussion issue
   - Updates PSP file with Discussion-To link
4. **Community discusses** in the auto-created issue
5. **Maintainer makes decision** and updates status accordingly

**Key benefit:** Authors only need to submit one PR with a Draft PSP - all status transitions and issue creation happen automatically upon merge.

## 🔍 Troubleshooting

### PSP Number Not Assigned
- Check that filename uses 0000 placeholder
- Ensure PR modifies files in `docs/psps/source/`
- Workflow processes files matching `psp-0000*.rst` and `psp-XXXXXX*.rst` patterns

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
