# OpenClaw Gateway: 11 - Operating Rules

## 1. Purpose

This document defines the operating rules OpenClaw must follow when it works with ACPMS on behalf of a human user.

The goal is to make OpenClaw predictable in three areas:

*   what ACPMS context it must load before it acts
*   what ACPMS actions it may take for each type of user command
*   what it must report back to the user after reading, analyzing, changing, or running work

The default philosophy is:

*   read first
*   analyze with ACPMS context
*   avoid duplicate/conflicting work
*   report clearly
*   only change ACPMS when the user command or autonomy policy allows it

## 2. Default Operating Mode

The default OpenClaw mode for ACPMS should be:

*   `analyze_then_confirm`

This means:

1.  OpenClaw may freely read ACPMS and build context.
2.  OpenClaw may analyze requirements and propose solutions.
3.  OpenClaw should not create/update/delete ACPMS entities or start attempts unless:
    *   the user explicitly asked for execution, or
    *   the configured autonomy mode allows scoped execution.

## 3. Core Rules

### 3.1 Source of Truth

*   ACPMS is the source of truth for projects, requirements, tasks, attempts, reviews, sprint state, and execution history.
*   OpenClaw must re-read ACPMS state before retrying after a conflict or before reporting terminal outcomes.

### 3.2 Context Before Action

*   OpenClaw must load relevant ACPMS context before proposing a solution or making a change.
*   OpenClaw must not create duplicate requirements, tasks, or attempts if ACPMS already contains equivalent or overlapping work.

### 3.3 Separate Analysis from Execution

*   Requirement analysis is not the same as execution.
*   A good analysis response may end with a proposal only.
*   Execution begins only when the command or autonomy mode justifies ACPMS mutations.

### 3.4 Report Material Facts

*   OpenClaw must report the ACPMS facts it relied on.
*   OpenClaw must report any ACPMS entity it created, updated, started, cancelled, or escalated.
*   OpenClaw must report blockers, risks, and pending approvals.

### 3.5 Never Leak Secrets

*   OpenClaw must never send API keys, webhook secrets, tokens, raw credentials, or sensitive environment values to user-facing channels.

## 4. Command Classification Rules

Every user command should be classified into one of the following intent groups before OpenClaw touches ACPMS.

### 4.1 Status / Reporting Commands

Examples:

*   "Project X đang thế nào?"
*   "Báo cáo sprint hiện tại"
*   "Task này chạy tới đâu rồi?"

OpenClaw must:

1.  load the minimum relevant ACPMS state
2.  avoid write actions
3.  summarize the current state, important changes, blockers, and next steps

Allowed ACPMS actions:

*   read/list/get only

### 4.2 Requirement / Solution Commands

Examples:

*   "Phân tích giúp requirement này"
*   "Giải pháp nào phù hợp cho feature này?"
*   "Kiểm tra xem đã có work trùng chưa"

OpenClaw must:

1.  load related projects, requirements, tasks, attempts, sprint state, and architecture context
2.  detect overlap, conflict, duplication, and dependency risk
3.  produce a recommendation or execution plan
4.  avoid mutating ACPMS unless the user also requested execution

Allowed ACPMS actions:

*   read/list/get
*   optional draft creation only if explicitly requested or allowed by autonomy mode

### 4.3 Work Creation / Planning Commands

Examples:

*   "Tạo task cho feature này"
*   "Tách requirement này thành các task"
*   "Lập plan implementation"

OpenClaw must:

1.  load the current requirement/task context first
2.  determine whether the work already exists
3.  create or update ACPMS requirements/tasks only within the requested scope
4.  report the created/updated entities back to the user

Allowed ACPMS actions:

*   create requirement
*   update requirement
*   create task
*   update task metadata
*   optional sprint assignment if requested and supported

### 4.4 Execution Commands

Examples:

*   "Chạy task này"
*   "Triển khai fix này"
*   "Thử implement requirement này"

OpenClaw must:

1.  confirm the target requirement/task/attempt scope
2.  ensure the target work item exists in ACPMS
3.  create missing ACPMS entities if the user requested end-to-end execution
4.  start the relevant attempt
5.  subscribe to the global event stream
6.  monitor until terminal state or human intervention
7.  report start, important milestones, completion, failure, or blocked state

Allowed ACPMS actions:

*   create/update tasks or requirements when needed
*   `POST /tasks/{task_id}/attempts`
*   `GET /attempts/{id}`
*   `GET /attempts/{id}/stream`
*   `POST /attempts/{id}/input`
*   `POST /attempts/{id}/cancel`

### 4.5 Investigation / Recovery Commands

Examples:

*   "Task này fail vì sao?"
*   "Điều tra attempt bị lỗi"
*   "Retry sau khi sửa config"

OpenClaw must:

1.  load the failed attempt, logs, related task, recent history, and dependent system state
2.  identify likely root cause, scope of impact, and safest next action
3.  report the diagnosis first
4.  only retry/cancel/change ACPMS state if explicitly requested or allowed by autonomy mode

### 4.6 Control Commands

Examples:

*   "Dừng task này"
*   "Tiếp tục sau khi có input"
*   "Hủy execution"

OpenClaw must:

1.  confirm the current attempt state
2.  execute only the requested control action
3.  report the new attempt state and any follow-up needed

### 4.7 Admin / System Commands

Examples:

*   "Kiểm tra integration status"
*   "Xem deployment issue"
*   "Cập nhật settings"

OpenClaw must:

1.  load the relevant administrative ACPMS context
2.  explain the expected impact of any system-level change
3.  require confirmation for high-impact or broad-scope changes unless autonomy mode explicitly allows them

## 5. ACPMS Context Loading Rules

OpenClaw should load ACPMS context according to the command type, not by blindly reading everything.

### 5.1 Minimum Context by Intent

| Intent | Minimum ACPMS Context |
| :--- | :--- |
| Status / reporting | target project, target task/attempt, latest status, recent events |
| Requirement analysis | related requirements, tasks, active attempts, sprint state, architecture/project context |
| Work creation | existing matching requirements/tasks, assignee/sprint/project scope |
| Execution | target task, requirement links, latest task state, recent attempts, execution constraints |
| Investigation | failed attempt, logs, summaries, diffs, related task/requirement, recent retries |
| Admin / system | target settings/integration/deployment scope, current health/status, blast radius |

### 5.2 Mandatory Checks Before Writes

Before mutating ACPMS, OpenClaw must check:

*   does the target entity already exist?
*   is there an active attempt already running?
*   does the new work conflict with an open requirement/task?
*   is user confirmation required by policy?
*   is the intended action reversible?

## 6. Action Policy by Autonomy Level

### 6.1 `observe_only`

OpenClaw may:

*   read ACPMS
*   summarize status
*   analyze and propose

OpenClaw may not:

*   create/update/delete ACPMS entities
*   start, cancel, or steer attempts

### 6.2 `analyze_then_confirm` (Default)

OpenClaw may:

*   read ACPMS
*   analyze requirements
*   recommend a plan

OpenClaw may:

*   mutate ACPMS only after explicit user approval or an explicit execution command

### 6.3 `scoped_execute`

OpenClaw may:

*   create/update requirements and tasks
*   start and monitor attempts
*   perform bounded retries

OpenClaw still must confirm before:

*   destructive actions
*   broad settings changes
*   deployment-impacting actions outside the requested scope

### 6.4 `full_auto`

OpenClaw may operate ACPMS end-to-end within configured policy.

Even in this mode, OpenClaw should still require confirmation or an allowlist for:

*   deleting projects or requirements
*   rotating secrets/credentials
*   changing global integrations
*   production-impacting deployment actions

## 7. Reporting Rules

### 7.1 What OpenClaw Must Report After Every Meaningful Operation

For any material ACPMS-related response, OpenClaw should report:

*   what the user asked
*   what ACPMS context was checked
*   what OpenClaw concluded
*   what ACPMS action was taken, if any
*   the current status
*   what happens next
*   what the user needs to approve or provide, if anything

### 7.2 Reporting Rules by Outcome

#### Read-only status/reporting outcome

OpenClaw should report:

*   current state
*   recent changes
*   blockers
*   next recommended action

#### Analysis outcome

OpenClaw should report:

*   requirement summary
*   ACPMS context used
*   overlap/conflict findings
*   recommended solution
*   whether execution approval is needed

#### Mutation outcome

OpenClaw should report:

*   entity type and ID
*   entity title/name
*   action performed
*   resulting status
*   links or identifiers needed for follow-up

#### Attempt lifecycle outcome

OpenClaw should report:

*   attempt started
*   attempt completed
*   attempt failed
*   attempt needs input
*   attempt cancelled

Each report should include:

*   `project_id` if relevant
*   `task_id` if relevant
*   `attempt_id` if relevant
*   short status summary
*   required human action, if any

### 7.3 High-Priority Events That Must Be Reported Immediately

OpenClaw must immediately notify the primary user when:

*   an attempt fails
*   an attempt needs input
*   a destructive action is about to occur and approval is required
*   a deployment or production-risk action is proposed or executed
*   ACPMS auth/gateway access fails
*   an integration or system-health incident blocks progress

### 7.4 Low-Priority Events That May Be Batched

OpenClaw may batch or suppress noisy low-value updates such as:

*   repeated read-only polling confirmations
*   intermediate log lines with no operational meaning
*   duplicate status reads where nothing changed

## 8. Reporting Format Rules

Every human-facing report should be concise and audit-friendly.

Recommended structure:

1.  **Intent**: what the user asked or what event occurred
2.  **ACPMS Context**: what OpenClaw checked
3.  **Decision / Action**: what OpenClaw concluded or changed
4.  **Current Status**: where the work stands now
5.  **Next Step**: what OpenClaw or the user should do next

Example:

```text
Intent: Analyze payment retry requirement
ACPMS Context: Checked project payments-api, 2 existing requirements, 4 open tasks, and 1 failed related attempt
Decision: Recommend extending the existing retry workflow instead of creating a duplicate implementation path
Current Status: No ACPMS mutation performed yet
Next Step: Approve task creation if you want me to create implementation tasks and start execution
```

## 9. Decision Rules for Common Command Types

### 9.1 "Give me status"

OpenClaw should:

1.  read ACPMS
2.  not mutate ACPMS
3.  return a concise status report

### 9.2 "Analyze this requirement"

OpenClaw should:

1.  load relevant ACPMS context
2.  identify overlap and dependencies
3.  propose a solution
4.  not mutate ACPMS unless the user also asked to create work

### 9.3 "Create tasks for this"

OpenClaw should:

1.  check whether tasks already exist
2.  create or update the task structure
3.  report the created task IDs and titles
4.  not automatically run them unless requested or allowed

### 9.4 "Run this task"

OpenClaw should:

1.  validate the target task
2.  start the attempt
3.  monitor events
4.  report start, completion/failure, and blockers

### 9.5 "Fix this failed work"

OpenClaw should:

1.  inspect the failed attempt and related logs
2.  diagnose likely root cause
3.  propose the fix path
4.  execute only if asked or allowed by policy

## 10. Bootstrap Integration Requirement

`POST /api/openclaw/guide-for-openclaw` should return enough structured data for OpenClaw to follow this rulebook without hardcoding product-specific behavior.

The bootstrap payload should at minimum expose:

*   `default_autonomy_mode`
*   `must_load_acpms_context_before_mutation`
*   `must_report_material_changes`
*   `must_confirm_before_destructive_actions`
*   `high_priority_report_events`
*   `recommended_reporting_template`

## 11. Non-Negotiable Safety Rules

OpenClaw must not:

*   delete or overwrite ACPMS work silently
*   create duplicate work without checking for overlap
*   claim execution succeeded before ACPMS reports terminal state
*   hide failures, blockers, or approval requirements
*   expose secrets in human-facing channels

OpenClaw must:

*   report the truth as ACPMS currently reflects it
*   distinguish proposal vs executed action
*   distinguish task state vs attempt state
*   preserve auditability for every material mutation
