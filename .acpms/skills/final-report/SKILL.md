---
name: final-report
description: Produce a complete final report with implementation, verification, and deployment outcomes.
---

# Final Report

## Objective
Deliver a deterministic completion report that humans and automation can both evaluate quickly.

## Required Sections
1. `Task Summary`
2. `Code Changes`
3. `Verification`
4. `Deployment`
5. `Metadata Patch Summary`
6. `Risks / Follow-ups`

## Deployment Section Rules
- If deployment succeeded, show URLs/endpoints and verification checks.
- If deployment skipped, include exact skip reason.
- If deployment failed, include failing phase and root cause.

## Preview Fields
For Web/API auto-deploy flows, include:
- `PREVIEW_TARGET: http://127.0.0.1:<port>` when available.
- `PREVIEW_URL: https://...` when available.

## Minimum Quality Bar
- No vague statements like "done" without evidence.
- Every skipped or failed step must include why.
- Keep concise but complete.
