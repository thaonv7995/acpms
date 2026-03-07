---
name: final-report
description: Produce a short final report that highlights outcome, verification, and next action only when needed.
---

# Final Report

## Objective
Deliver a short completion report that a human can scan in a few seconds.

## Required Format
- Use `## Final Report`
- Then use only the sections that matter for this task
- Prefer 2 to 4 bullets total
- Keep the whole report to about 6 lines when possible

## Preferred Content
- `Done:` what changed in one sentence
- `Verified:` only the checks that actually matter
- `Deploy:` only when preview/deploy is relevant
- `Next:` only when there is a real blocker, risk, or follow-up

## Rules
- Omit empty or irrelevant sections entirely
- Do not include `Metadata Patch Summary` unless metadata itself was the task
- Do not include a generic risk/follow-up section when there is no real issue
- Do not list commands verbatim; summarize outcomes instead
- For docs-only or tiny tasks, 2 bullets is enough
- If deploy/preview succeeded, include the final URL only
- If deploy/preview was skipped, give a one-line reason only
- If a check failed because of a pre-existing unrelated issue, mention it briefly in `Next:`
- After the final report, stop. Do not repeat the same summary in extra prose

## Minimum Quality Bar
- No vague statements like "done" without evidence.
- Every skipped or failed step must include why.
- Keep concise and concrete.
