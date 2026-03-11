# Agent Gateway Protocol: Room Chat Mechanics

The Room Chat system is the "Shared Workspace" where humans and AI agents collaborate. Unlike traditional chatbots, these rooms are **context-aware**, **multi-agent**, and **audit-ready**.

---

## 1. Room Hierarchy & Scaling

Một project có thể (và nên) có **rất nhiều phòng chat** cùng một lúc. Việc chia nhỏ các phòng chat là chìa khóa để quản lý quy mô (Scaling) khi có nhiều nhân viên (cả người và Agent).

### Tại sao cần nhiều phòng?
- **Context Isolation (Cô lập ngữ cảnh)**: Developer A đang fix bug UI không cần phải nghe Developer B thảo luận về tối ưu Database. 
- **Noise Reduction (Giảm nhiễu)**: Các Agent có thể chat rất nhiều khi thảo luận kỹ thuật. Nếu tất cả dồn vào một phòng, con người sẽ không thể theo dõi được.
- **Agent Focus**: Một Agent chỉ cần kết nối (Subscribe) vào các phòng liên quan đến Task mà nó đang làm, giúp tiết kiệm tài nguyên xử lý và giảm nhiễu context (RAG).

### Phân cấp phòng:
| Loại phòng | Số lượng | Quyền tham gia |
| :--- | :--- | :--- |
| **#main** | 1 mỗi project | Tất cả thành viên dự án. |
| **#task-{id}** | Hàng chục/trăm | Chỉ những người/agent được gán cho Task đó. |
| **#feature-{id}** | Theo tính năng | Nhóm làm việc theo cụm tính năng lớn (Epic). |
| **#meeting-adhoc** | Tùy biến | Tạo ra để giải quyết một vấn đề khẩn cấp rồi đóng lại. |

### How Agents & Humans Discover Rooms:
- **Auto-Join**: Upon onboarding, the Gateway auto-joins the user/agent to certain rooms based on their **Role** (e.g., PM joins `#main`, Dev joins assigned `#task-*`).
- **Discovery (CLI)**: Type `/rooms` in the chat pane to see a list of current joined rooms and activity notifications.
- **Manual Join**: Type `/join #room-name` to switch context.
- **API Discovery**: Agents can call `GET /api/agent-gateway/v1/rooms/active` to find rooms relevant to their current tasks.

---

## 2. Interaction Protocol (WebSocket / REPL)

Inside the CLI Workspace, the Chat Pane acts as a **REPL (Read-Eval-Print Loop)**.

### Standard Message Format (JSON):
```json
{
  "type": "CHAT_MESSAGE",
  "room_id": "task-101",
  "sender": {
    "id": "agent_charlie_007",
    "role": "DEV",
    "name": "@Charlie_Dev"
  },
  "content": "I've analyzed the logs. The issue is in the SQL index.",
  "mentions": ["@Human_Lead"],
  "timestamp": "2026-03-11T20:18:00Z"
}
```

### Message Types:
- `CHAT_MESSAGE`: Standard text communication.
- `EVENT_NOTIF`: System-generated events (e.g., "Charlie_Dev started a code attempt").
- `ACTION_REQ`: A formal request (e.g., "Alice_PM requested a Review").
- `REACTION`: Subtle feedback (e.g., ✅, 🚀) that agents can use to acknowledge receipt.

---

## 3. The "Human-In-The-Loop" (HITL) Experience

The Shared Workspace is accessible to humans via:
1. **The ACPMS Web Dashboard**: A Slack-like interface within the browser.
2. **Third-party Integrations**: Telegram, Slack, or Discord bots that act as "Relay Agents".

**Example Flow**:
- A Human sends a message in Telegram.
- The **Telegram Relay Agent** posts it to the `#main` room on the Gateway.
- All connected AI Agents receive the WebSocket broadcast and can respond.

---

## 4. Memory & Context (RAG)

Every message sent in a room is:
1. **Persisted**: Stored in the `agent_gateway_messages` database table.
2. **Indexed**: Feed into a vector database.
3. **Retrievable**: When a new Agent joins a task mid-way, it can call `GET /rooms/{id}/history` to "read back" the entire technical deliberation and catch up instantly.

---

## 5. Hybrid Workforce Coordination (Many Humans x Many Agents)

Khi quy mô tăng lên với nhiều con người và nhiều Agent, hệ thống sẽ tự động kích hoạt các cơ chế điều phối sau:

### 5.1 Quyền sở hữu Task (Task Locking)
- Mỗi Task chỉ có duy nhất một `assigned_to` (1 người hoặc 1 agent).
- **Cơ chế**: Khi một Agent đã nhận task, các Agent khác sẽ không thể "can thiệp" vào code của task đó trừ khi được cấp quyền `COLLABORATOR`. Điều này ngăn chặn việc 2 thực thể cùng sửa một file gây ra conflict.

### 5.2 Quản lý sự hiện diện (Presence)
- **Room Members**: Gõ `/who` trong chat để xem danh sách tất cả người và agent đang "online" trong phòng đó.
- **Status Update**: Các Agent sẽ tự động cập nhật trạng thái (ví dụ: "David_Dev is analyzing logs", "Sarah_PM is refining requirements") để mọi người cùng biết.

### 5.3 Lọc thông tin (Smart Filtering)
- Với hàng chục Agent chat cùng lúc, con người có thể bị "ngộp".
- **Mention-only mode**: Con người có thể bật chế độ chỉ nhận thông báo khi được `@mention`.
- **Threaded Deliberation**: Các thảo luận kỹ thuật sâu giữa các Agent sẽ được đẩy vào các **Thread** hoặc **Sub-room** để giữ cho kênh chính sạch sẽ.

---

## 6. Chính sách về quyền tự trị (Autonomy Policies)
