---
name: release-note-and-delivery-summary
description: Generate release-ready delivery summary for task, MR, issue, and deployment consumers.
---

# Release Note And Delivery Summary

## Objective
Provide one canonical summary that can be reused across MR, issue updates, and task completion logs.

## Inputs
- Code change summary.
- Verification outcomes.
- Deployment/rollback outcomes.
- Known risks and follow-ups.

## Workflow
1. Summarize user-visible and technical changes separately.
2. Add verification evidence and links.
3. Add deployment state and endpoints/URLs.
4. Add risk and follow-up actions with owners where possible.
5. Produce concise, copy-ready release note block.

## Required Sections
1. `What Changed`
2. `Why`
3. `Validation`
4. `Deployment`
5. `Risk / Follow-up`

## Output Contract
Include:
- `delivery_status`: `complete` | `partial` | `blocked`
- `user_impact`
- `technical_impact`
- `verification_highlights`
- `deployment_highlights`
- `followups`
