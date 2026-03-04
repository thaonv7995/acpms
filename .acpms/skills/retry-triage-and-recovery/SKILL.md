---
name: retry-triage-and-recovery
description: Classify attempt failures, decide retriable actions, and execute minimal safe recovery path.
---

# Retry Triage And Recovery

## Objective
Improve retry success rate by distinguishing transient failures from deterministic failures.

## Inputs
- Previous attempt error and logs.
- Retry policy (max retries, auto-retry enabled).
- Current repository and environment state.

## Workflow
1. Classify failure category: infra/transient/config/code/permission.
2. Decide retriable vs non-retriable.
3. Apply smallest recovery action first.
4. Re-run only required validation path.
5. Record recovery outcome and whether further retry is useful.

## Decision Table
| Failure Type | Retriable | Recovery Action |
|---|---|---|
| Network timeout / transient infra | Yes | Retry with backoff; no code changes first. |
| Missing secret/config | Usually No | Stop and request config fix. |
| Deterministic test/build failure | No until fixed | Apply code/config fix before retry. |
| Permission/auth denied | No | Escalate credentials/permissions issue. |

## Guardrails
- Do not burn retries on known deterministic failures.
- Keep retry edits isolated and traceable.

## Output Contract
Include:
- `retry_classification`
- `retry_decision`: `retry_now` | `retry_later` | `do_not_retry`
- `recovery_actions`
- `recovery_result`
