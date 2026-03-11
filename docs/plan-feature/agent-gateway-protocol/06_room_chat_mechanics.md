# Agent Gateway Protocol: Room Chat Mechanics

The Room Chat system is the "Shared Workspace" where humans and AI agents collaborate. Unlike traditional chatbots, these rooms are **context-aware**, **multi-member**, and **audit-ready**.

---

## 1. Room Hierarchy & Scaling

A project can, and usually should, have many rooms at the same time. Splitting collaboration across rooms is the key to scaling a hybrid workforce of humans and agents.

### Why many rooms are needed

- **Context Isolation**: A developer fixing a UI bug should not have to read database optimization discussion.
- **Noise Reduction**: Agents may exchange many technical messages. Keeping them out of the main room protects human attention.
- **Member Focus**: A project member only needs to subscribe to rooms relevant to current work, reducing context noise and token cost.

### Room hierarchy

| Room Type | Typical Count | Participation |
| :--- | :--- | :--- |
| **#main** | 1 per project | All project members |
| **#task-{id}** | tens or hundreds | Assigned members and invited collaborators |
| **#feature-{id}** | per major feature | Members working on the same epic or feature cluster |
| **#meeting-*** | as needed | Temporary or persistent coordination rooms |

### How members discover rooms

- **Auto-Join**: After a principal becomes a project member, ACPMS auto-joins it to baseline rooms according to membership role and policy. Task rooms are auto-joined when the member is assigned or invited.
- **Discovery (CLI)**: Type `/rooms` in the chat pane to see joined rooms and activity notifications.
- **Manual Join**: Type `/join #room-name` to switch context.
- **API Discovery**: Agents can call `GET /api/agent-gateway/v1/rooms/active` to find rooms relevant to current memberships and assignments.

---

## 2. Interaction Protocol (WebSocket / REPL)

Inside the CLI Workspace, the chat pane acts as a REPL (Read-Eval-Print Loop).

### Standard Message Format (JSON)

```json
{
  "type": "CHAT_MESSAGE",
  "room_id": "task-101",
  "sender": {
    "principal_id": "principal_charlie_007",
    "principal_type": "agent",
    "project_role": "DEV",
    "name": "@Charlie_Dev"
  },
  "content": "I've analyzed the logs. The issue is in the SQL index.",
  "mentions": ["@Human_Lead"],
  "timestamp": "2026-03-11T20:18:00Z"
}
```

### Message Types

- `CHAT_MESSAGE`: Standard text communication
- `EVENT_NOTIF`: System-generated events such as "Charlie_Dev started a code attempt"
- `ACTION_REQ`: A formal request such as "Alice_PM requested a review"
- `APPROVAL_REQ`: A review or approval gate that expects explicit human action
- `HANDOFF`: Structured status transfer between project members
- `REACTION`: Lightweight acknowledgement such as `+1`, `ACK`, or `DONE`

### Agent-to-Agent Collaboration

Agents must be able to talk directly to each other inside the same room or thread when solving project problems.

Typical examples:

- a `BA` agent asks a `DEV` agent whether a requirement is technically feasible
- a `DEV` agent asks a `QA` agent for a failure reproduction path
- a `PM` agent asks a `BA` agent to summarize scope impact before changing priority

This inter-agent chat should be treated as normal Workspace collaboration, not as a special side-channel.

However, ACPMS should still enforce a few guardrails:

- prefer threads for deep technical back-and-forth
- keep the discussion scoped to the room's task, feature, or decision
- require a short summary or handoff back into the main room when the discussion reaches a conclusion
- prevent infinite ping-pong between agents by combining role policy, task ownership, and token controls

---

## 3. The Human-In-The-Loop (HITL) Experience

The Shared Workspace is accessible to humans via:

1. **The ACPMS Web Dashboard**: A Slack-like interface inside the browser
2. **The CLI Workspace**: A room feed beside the coding pane
3. **Third-party Integrations**: Telegram, Slack, or Discord bots acting as relay agents

**Example Flow**:

- A human sends a message in Telegram.
- The **Telegram Relay Agent**, already attached as a project member, posts it to the `#main` room.
- All connected project members receive the broadcast and can respond.

---

## 4. Memory & Context (RAG)

Every message sent in a room is:

1. **Persisted**: Stored in the `agent_gateway_messages` table
2. **Indexed**: Fed into a retrieval system or vector database
3. **Retrievable**: When a new member joins a task late, it can call `GET /rooms/{id}/history` to replay prior deliberation

---

## 5. Hybrid Workforce Coordination

As the number of humans and agents grows, ACPMS should activate the following coordination mechanics.

### 5.1 Task Ownership (Task Locking)

- Each task has a single primary `assigned_member_id`
- If that member is an agent, other agents cannot intervene on the task by default unless explicitly added as collaborators
- This prevents two autonomous members from editing the same work item without coordination
- Task ownership should be implemented as a renewable lease, not an infinite lock
- Lease expiry or stale ownership should allow a controlled takeover or reassignment path
- ACPMS should support a `request_to_take_over` flow when another member needs ownership while the current owner is unavailable
- For code execution, agent work should land in a git branch, patch set, or PR-like staging area so concurrent human changes can be reconciled safely

### 5.2 Presence

- `/who` shows all humans and agents online in the room
- Each member can expose status such as:
  - `analyzing logs`
  - `refining requirement`
  - `waiting for review`

### 5.3 Smart Filtering

- Humans can switch to mention-only mode
- Deep technical discussion can be pushed into threads or sub-rooms
- Members can favorite important rooms such as `#main` or a current `#task-*`

### 5.4 Inter-Agent Discussion Patterns

- Agents may consult each other in the same room when the problem crosses role boundaries
- The primary assigned member remains responsible for the final task outcome unless ownership is transferred
- Secondary agents act as collaborators, reviewers, or subject-matter specialists
- When a discussion becomes long, ACPMS should encourage thread use and eventual summary publication

### 5.5 Handoffs, Approvals, and Deadlock Recovery

- `APPROVAL_REQ` and `HANDOFF` states should have explicit SLA timers
- If the expected human or agent owner does not respond within the timeout window, ACPMS should escalate automatically
- Escalation targets may include:
  - a backup approver
  - the project owner
  - a manager role
  - a system alert or inbox notification
- Deadlocked threads should not remain blocked forever; ACPMS should surface `blocked_too_long` alerts and propose delegation or reassignment

---

## 6. Autonomy Policies

Autonomy is enforced at the **project membership** layer, not just at the global agent identity layer.

This allows the same agent principal to:

- operate with high autonomy in one project
- operate in confirm-first mode in another
- be removed from one project without losing global registration

See `10_room_message_delivery_and_local_agent_loop.md` for the detailed runtime delivery model, local agent triage loop, and token-control strategy for busy rooms.
