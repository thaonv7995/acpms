---
name: retry-triage-and-recovery
description: Classify attempt failures, decide retriable actions, and execute minimal safe recovery path.
---

# Retry Triage And Recovery

## Objective
Improve retry success rate by separating transient failures from deterministic
ones and by choosing the smallest recovery action that can actually change the
outcome.

## When This Applies
- A previous attempt failed and ACPMS is deciding whether to retry
- An auto-fix or retry loop needs a narrower recovery action
- The task should avoid burning retries on the same deterministic failure

## Inputs
- Previous attempt error and logs
- Retry policy (max retries, auto-retry enabled)
- Current repository and environment state

## Workflow
1. Classify the failure: infra, transient, config, code, permission, or state.
2. Decide whether it is retriable now, retriable later, or not retriable.
3. Apply the smallest recovery action first.
4. Re-run only the validation path needed to prove recovery.
5. Record whether another retry has a real chance of success.

## Decision Rules
| Failure Type | Retriable | Recovery Action |
|---|---|---|
| Network timeout or transient infra | Yes | Retry with backoff before changing code. |
| Missing secret or config | Usually no | Stop and request configuration fix. |
| Deterministic test/build failure | No until fixed | Apply code or config fix before retry. |
| Permission/auth denied | No | Escalate credential or permission issue. |

## Output Contract
Emit:
- `retry_classification`
- `retry_decision`: `retry_now` | `retry_later` | `do_not_retry`
- `recovery_actions`
- `recovery_result`

## Related Skills
- `task-preflight-check`
- `env-and-secrets-validate`
- `verify-test-build`
- `review-handoff`
