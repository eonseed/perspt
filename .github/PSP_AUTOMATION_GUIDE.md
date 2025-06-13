# PSP GitHub Automation Guide

This document explains the GitHub automation workflows for managing Perspt Specification Proposals (PSPs).

## 🚀 Quick Start

**For PSP Authors:**
1. Create a PR with your Draft PSP file named `psp-0000-your-title.rst`
2. **Check the "PSP" checkbox** in the pull request template
3. Use `PSP: 0000` in the file header 
4. Set `Status: Draft`
5. Fill out the PSP Information section in the PR template
6. Submit PR for maintainer review

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

**Triggers:** PSP file changes in PRs, PSP proposal issues, PSP checkbox in PR template
**Functions:**
- Detects PSP-related pull requests via file changes OR checkbox selection
- Validates consistency between PSP checkbox and file modifications
- **Adds guidance comments** to PSP PRs acknowledging template selection
- Assigns PSP numbers automatically (replaces 0000 placeholders)
- Automatically transitions Draft PSPs to Discussion status upon merge
- Creates discussion issues for community feedback
- Updates PSP files with Discussion-To links
- Adds helpful comments to PSP proposal issues

**PSP Detection Logic:**
- **Primary**: PSP checkbox checked AND PSP files present (required for PSP processing)
- **Secondary**: PSP files modified without checkbox (processes as PSP with warning)
- **Validation**: Checkbox checked without files = treated as general PR with helpful comment
- **Feedback**: Clear comments explain validation results and next steps

### 2. PSP Documentation (`psp-docs.yml`)

**Triggers:** Changes to PSP documentation (`docs/psps/**`), manual dispatch
**Functions:**
- Builds PSP documentation with Sphinx using hatchling build system
- Generates workflow diagrams from .dot files with Graphviz
- Validates PSP document format and required metadata fields
- Checks for duplicate PSP numbers across all documents
- Validates 6-digit PSP number format requirements
- Uploads documentation artifacts to GitHub Pages
- **Template-Independent**: Works with any PSP file changes regardless of PR template

### 3. PSP Status Management (`psp-status-management.yml`)

**Triggers:** Manual dispatch, issue closures with PSP labels, scheduled (weekly)
**Functions:**
- **Manual status updates** by maintainers via workflow dispatch
- **Automatic status updates** based on discussion issue outcomes and labels
- **Label-based automation**: `psp-accepted`, `psp-rejected`, `psp-withdrawn`
- **Stale PSP detection**: Identifies PSPs in Discussion >60 days
- **Maintenance automation**: Creates issues for stale PSP review
- **Template-Independent**: Works with existing PSP files regardless of how they were created

## 📝 Pull Request Template

### Unified Template Approach
- **Single template** handles both General and PSP pull requests
- **PSP Type Selection**: Check `- [x] **PSP**` checkbox to trigger PSP automation
- **PSP Information Section**: Additional fields for PSP-specific details
- **Automation Trigger**: Checkbox selection determines workflow routing

### Template Sections for PSP PRs
- **Pull Request Type**: Must check PSP checkbox
- **PSP Information**: PSP number, title, type, related issues
- **PSP Status**: Draft/Discussion/Revision checkboxes  
- **PSP Content Checklist**: Validation checklist for document completeness
- **Implementation Details**: Planning and feasibility notes

## � Issue Templates

### PSP Proposal (`psp_proposal.yml`)
- For initial PSP ideas and community discussion
- Captures all essential information before formal PSP creation
- Auto-assigns labels and provides guidance

### PSP Discussion (`psp_discussion.yml`)
- For discussing specific PSP documents
- Auto-created by workflows when PSP PRs are submitted
- Links to the PSP document and provides discussion guidelines

## �🔄 PSP Lifecycle Automation

### 1. Initial Proposal (Optional)
```
User creates PSP proposal issue → Auto-comment with guidance
```

### 2. PSP Document Creation
```
User creates PR with:
├── Draft PSP file (using 0000 placeholder)  
├── PSP checkbox checked in PR template
└── PSP Information section filled out
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
**Automatic:** Workflows detect 0000 placeholders and assign next available number
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

The PSP automation provides a streamlined process with improved detection:

1. **Author creates PR** with:
   - Draft PSP file (using `psp-0000-title.rst`)
   - **PSP checkbox checked** in PR template
   - PSP Information section completed
   
2. **Automation detects PSP** via:
   - Primary: PSP checkbox selection
   - Secondary: PSP file modifications
   - Validates consistency between both
   
3. **Maintainer reviews and merges** the PR

4. **Automation automatically:**
   - Assigns next available PSP number
   - Updates filename and content
   - Changes status from "Draft" to "Discussion"
   - Creates discussion issue
   - Updates PSP file with Discussion-To link
   
5. **Community discusses** in the auto-created issue

6. **Maintainer makes decision** and updates status accordingly

**Key benefits:** 
- **Dual detection**: Checkbox + file changes for reliability
- **Better UX**: Clear template guidance for authors
- **Consistency validation**: Warns about mismatched selections
- **Single PR workflow**: Authors only need one PR submission

## 🔍 Troubleshooting

### PSP Not Detected / Treated as General PR
- **Checkbox only, no files**: Gets helpful comment explaining how to add PSP files
- **Check file location**: PSP files must be in `docs/psps/source/`
- **Check filename**: Must match `psp-*.rst` pattern (e.g., `psp-0000-title.rst`)
- **Check template**: Ensure PSP checkbox is checked for intended PSP PRs
- **Review workflow logs**: Check Actions tab for detailed detection logic

### PSP Number Not Assigned
- Check that filename uses 0000 placeholder
- Ensure PSP checkbox is checked in PR template
- Verify PR modifies files in `docs/psps/source/`
- Workflow processes files matching `psp-0000*.rst` pattern (also supports legacy `psp-XXXXXX*.rst`)

### Validation Scenarios
- **✅ Valid PSP**: Checkbox checked + PSP files present → Full PSP processing
- **⚠️ Files only**: PSP files present but checkbox unchecked → PSP processing with warning
- **⚠️ Checkbox only**: Checkbox checked but no PSP files → General PR with guidance comment
- **ℹ️ Neither**: No checkbox, no files → Standard general PR processing

### Discussion Issue Not Created
- Verify PSP metadata is properly formatted
- Check that PSP checkbox was checked in original PR
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
