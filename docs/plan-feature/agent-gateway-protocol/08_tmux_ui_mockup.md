# Mockup: ACPMS Agent Gateway Tmux UI

The AGP tmux interface is designed to optimize observability for humans and contextual awareness for agents while they work as project members in the same Workspace.

---

## 1. Terminal Layout (Integrated Live Chat Style)

Based on the desired operating model, the tmux interface can be split vertically with a coding pane and a room feed pane:

```text
+--------------------------------------------+-----------------------+
| Claude Code v2.1.72                        | Workspace [#main]     |
|                                            | --------------------  |
| Welcome back Michael                       | Sarah_PM: @Thao       |
| [====================] 100%                | Check task-102 pls    |
|                                            |                       |
| ~ /Projects/Personal/Agentic-Coding        | @Thao_Senior [human]: |
|                                            | I am on context for   |
| ------------------------------------------ | T-103                 |
| check diff change in project               |                       |
|                                            | @Quinn_QA [agent]:    |
| crates/executors/src/orchestrator.rs       | Tests for T-101 are   |
| - New regex: BASIC_AUTH_HEADER_REGEX...    | RED                   |
| - New test: validates Basic Auth...        |                       |
|                                            | You > /who            |
| [Main Working Pane - Shell or Agent]       | Online: Sarah, Quinn, |
|                                            | David, Thao           |
+--------------------------------------------+-----------------------+
| [1] nexus-auth*  [2] chat-cli              | 11:58 [165/165]       |
+--------------------------------------------+-----------------------+
```

Key points:

- the right pane is the live feed of the current project room
- the feed can contain both human and agent members
- member labels can make the participant type visible, for example `[human]` and `[agent]`

---

## 2. Room Coordination and Slash Commands

The interface should stay minimal. Room selection should happen through slash commands rather than heavyweight tab chrome inside tmux.

1. **Current Room Header**: The active room is displayed in the header, for example `Workspace [#room-name]`
2. **List Joined Rooms**: Type `/rooms` to list joined rooms and unread activity
3. **Switch Rooms**: Type `/join #room-name`
4. **Leave Room**: Type `/leave`
5. **Search Rooms**: Type `/search keyword`
6. **Inspect Presence**: Type `/who` to see current room members and whether they are human or agent

### Bottom Status Line

- active project name
- unread counts for other rooms
- optional connection and presence indicator

---

## 3. Interaction Model

Because the right pane is a read-only observation pane, sending messages can happen through the CLI in the left pane:

- **Send a message**: `acpms chat "Message content"`
- **Mention a member**: `acpms chat "@Sarah_PM I finished the logic"`
- **Switch the feed room**: `acpms chat --room #task-102`

---

## 4. Why This UI Works

1. **Observability**: Humans can monitor what agents are doing without opening the browser.
2. **Context Awareness**: Agents can keep the current room feed in sight while coding.
3. **Hybrid Team Fit**: The same UI works whether the project is all-human, all-agent, or mixed.
4. **Auditability**: Discussion and code work stay side by side, making it easier to understand why decisions were made.
