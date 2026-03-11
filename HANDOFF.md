# Handoff: Attempt runtime tracking and elapsed-time visibility

## Summary
Added attempt runtime tracking and elapsed-time visibility: start/end timestamps are used (API already provides `started_at` / `completed_at`), elapsed minutes are computed in real time for running attempts, and elapsed/status is shown in task and attempt UIs.

## Changed files

| File | Change |
|------|--------|
| `frontend/src/utils/elapsedTime.ts` | **New.** Helpers: `getElapsedMs`, `getElapsedMinutes`, `formatElapsed(start, end?)` for human-readable duration. |
| `frontend/src/hooks/useElapsedRealtime.ts` | **New.** Hook: returns formatted elapsed string, ticks every 10s when running for live updates. |
| `frontend/src/types/task-attempt.ts` | No change. Already has `started_at`, `completed_at`, `ended_at`. |
| `frontend/src/components/panels/TaskPanel.tsx` | Map `ended_at` from API `completed_at`; time column uses `AttemptTimeCell`: running → "Running Xm Ys" (live), completed/failed → duration, else time ago. |
| `frontend/src/components/panels/AttemptHistoryPanel.tsx` | Use `formatElapsed` and `ended_at ?? completed_at` for completed/failed duration; for running, show "Running for X" via `AttemptRunningElapsed` (useElapsedRealtime). |
| `frontend/src/components/timeline-log/TimelineHeader.tsx` | New prop `attemptStartedAt`; when status is running, show "(Xm Ys)" next to "Running" (live). |
| `frontend/src/components/timeline-log/TimelineLogDisplay.tsx` | New optional prop `attemptStartedAt`, passed to `TimelineHeader`. |
| `frontend/src/components/panels/TaskAttemptPanel.tsx` | Pass `attempt.started_at` as `attemptStartedAt` to `TimelineLogDisplay`. |
| `frontend/src/components/panels/AttemptSwitcher.tsx` | When current attempt is running and has `started_at`, show "(Xm Ys)" in trigger button (live). |
| `frontend/src/pages/project-tasks/use-attempt-data.ts` | Map `ended_at` from API `completed_at` for selected and sorted attempts. |

## Scope notes
- **Backend:** No changes. API already returns `started_at` and `completed_at`; orchestrator sets them when attempt starts/finishes.
- **Generated API (orval):** Unchanged. `TaskAttemptDto` already has `started_at`, `completed_at`.
- **Other `TimelineLogDisplay` call sites** (AgentStreamPage, AgentFeed, RequirementBreakdownModal, TimelineLogExample): Do not pass `attemptStartedAt`; elapsed in header only when parent passes it (e.g. TaskAttemptPanel).

## Verification
- **Lint:** No linter errors in modified files (read_lints).
- **Build/tests:** Not run in this environment (no `node_modules` / npm install in progress). **Reviewer should run:**  
  `cd frontend && npm install && npm run build && npm run test -- --run`

## Known risks / reviewer actions
1. **Realtime tick:** `useElapsedRealtime` uses a 10s interval; fine for “elapsed minutes” visibility. If you prefer 1m or 30s, change `TICK_MS` in `frontend/src/hooks/useElapsedRealtime.ts`.
2. **Timezone:** Elapsed uses client `Date.now()` and ISO timestamps from API; no server TZ change.
3. **Manual test:** Open a task with a running attempt and confirm: Task panel table shows "Running Xm Ys", Attempt History shows "Running for Xm Ys", timeline header shows "Running (Xm Ys)", attempt switcher shows "(Xm Ys)".

## Reviewer actions
- Run `npm install`, `npm run build`, `npm run test -- --run` in `frontend`.
- Optionally run the full app and confirm elapsed visibility in the places above.
- No commit or push per execution rules.
