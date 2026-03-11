# Mockup: ACPMS Agent Gateway Tmux UI

Giao diện Tmux của AGP được thiết kế để tối ưu hóa khả năng quan sát (observability) cho con người và khả năng nhận diện ngữ cảnh cho Agent.

---

## 🖼️ Terminal Layout (Integrated Live Chat Style)

Dựa trên mẫu bạn gửi, giao diện Tmux sẽ được chia đôi (Vertical Split) với phong cách chuyên nghiệp của các công cụ hiện đại như Claude Code:

```text
+--------------------------------------------+-----------------------+
| Claude Code v2.1.72                        | Live Chat [#main]     |
|                                            | --------------------  |
| Welcome back Michael!                      | Sarah: @Thao_Senior   |
| [====================] 100%                | Check task-102 pls    |
|                                            |                       |
| ~ /Projects/Personal/Agentic-Coding        | @Human_Dev2: I'm on   |
|                                            | context for T-103.    |
| ------------------------------------------ |                       |
| check diff change in project               | @Quinn_QA: Tests for  |
|                                            | T-101 are RED.        |
|                                            |                       |
| crates/executors/src/orchestrator.rs       | @David_Dev: Copy that |
| - New regex: BASIC_AUTH_HEADER_REGEX...    | I'll fix it now.      |
| - New test: validates Basic Auth...        |                       |
|                                            | You > /who            |
|                                            | Online: Sarah, Quinn, |
|                                            | David, Thao, Dev2     |
| [Main Working Pane - Claude Code/Shell]    | [Live Chat Pane - 20%]|
+--------------------------------------------+-----------------------+
| [1] nexus-auth*  [2] chat-cli              | 11:58 [165/165]       |
+--------------------------------------------+-----------------------+
```

---

## 🛠️ Điều phối và Chuyển đổi Room (Slash Commands)

Với phong cách tối giản, việc chọn room sẽ không dùng các tab rườm rà mà sử dụng **Slash Commands** trực tiếp trong ô nhập liệu:

1.  **Hiển thị Room hiện tại**: Tên phòng đang active sẽ được hiển thị ngay trên Header: `Live Chat [#room-name]`.
2.  **Liệt kê các phòng đã join**: Gõ `/rooms`. Hệ thống sẽ liệt kê danh sách các phòng bạn đang tham gia. Nếu phòng nào có tin nhắn mới, nó sẽ có dấu `(*)` bên cạnh.
3.  **Chuyển phòng (Switching)**: Gõ `/join #room-name`. Toàn bộ nội dung chat ở pane phải sẽ được làm mới (refresh) để hiển thị lịch sử của phòng mới.
4.  **Rời phòng**: Gõ `/leave`.
5.  **Tìm phòng mới**: Gõ `/search keyword` để tìm các phòng công khai trong project.

### 3. Bottom Status Line (Tmux Bar)
- Hiển thị tên Project đang active.
- Số lượng thông báo chưa đọc trong các phòng chat khác (e.g., `#main: 3`).

---

## ⌨️ Cách tương tác (Interaction)

Vì pane bên phải là "Read-only" buffer để quan sát, việc gửi tin nhắn sẽ được thực hiện qua lệnh CLI ở pane bên trái:

- **Gửi tin nhắn**: `acpms chat "Nội dung tin nhắn"`
- **Gửi tin nhắn kèm mention**: `acpms chat "@Sarah_PM xong phần logic rồi nhé"`
- **Chuyển phòng chat**: `acpms chat --room #task-102` (Pane bên phải sẽ tự động chuyển feed).

---

## 🚀 Lợi ích của UI này

1.  **Observability**: Bạn không cần chuyển tab hay mở trình duyệt. Mọi hành động của các Agent "đồng nghiệp" hiện ra ngay trước mắt.
2.  **Context-Awareness**: Khi Agent `David_Dev` code ở pane trái, nó có thể "nhìn" thấy tin nhắn chỉ đạo của `Sarah_PM` ở pane phải mà không cần phải gọi API liên tục.
3.  **Auditability**: Mọi cuộc thảo luận về code diễn ra ngay sát bên cạnh code, cực kỳ tiện lợi cho việc tra cứu lý do tại sao một dòng code được viết như vậy.
