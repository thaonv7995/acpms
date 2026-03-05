# Ephemeral Sandbox: 02 - Execution Flow (Docker Volume Mount)

## 1. Chuẩn bị (Pre-Execution)

Khi một Agent session được trigger thông qua API hoặc UI (ví dụ: `POST /api/sessions/trigger`), Orchestrator sẽ không lập tức gọi tiến trình con (như `npx @anthropic-ai/claude-code`) như trước đây.

1.  **Xác định Workspace**: Orchestrator xác định đường dẫn vật lý trên Host OS tới thư mục worktree của dự án, ví dụ: `/app/data/worktrees/project_xyz`.
2.  **Chuẩn bị Image**: Orchestrator chọn Docker image phù hợp (ví dụ: `agentic/sandbox-node-20:latest`).
3.  **Tạo Container Tạm (Ephemeral)**:
    Orchestrator tạo một Docker container với các cờ (flags) quan trọng:
    *   `--rm`: Tự động xóa container khi tiến trình kết thúc.
    *   `-v /app/data/worktrees/project_xyz:/workspace`: Mount thư mục dự án vào `/workspace`.
    *   `-w /workspace`: Đặt Working Directory khởi điểm là `/workspace`.
    *   `--user 1000:1000`: Chạy dưới UID/GID không có quyền Root.
    *   `--network host` (hoặc network riêng biệt tùy cài đặt bảo mật).
    *   `--memory="2g" --cpus="2"`: Ngăn Agent ăn hết RAM/CPU.

## 2. Chạy Process nội bộ (Inter-process Communication)

### Phương pháp 1: Lệnh `docker run -i` trực tiếp
Thay vì spawn `claude-code`, Orchestrator spawn một tiến trình:
`docker run -i ... agentic/sandbox-node-20 npx @anthropic-ai/claude-code`

*   **Ưu điểm**: Orchestrator (Host) vẫn nối `stdin`, `stdout`, `stderr` trực tiếp vào tiến trình `docker CLI`. Mọi thứ (Streaming API, HitL, Database logging) không cần thiết kế lại quá nhiều.
*   **Nhược điểm**: `docker CLI` đôi khi nuốt mất các ký tự điều khiển (TTY), cần cẩn thận khi truyền luồng dữ liệu.

### Chỉnh sửa Code tại `crates/executors/src/orchestrator.rs`
Hàm `run_claude_process` hoặc `trigger_session` cần xử lý biến môi trường `ACPMS_USE_SANDBOX=true` để bọc chuỗi lệnh cần chạy (cmd + args) vào bên trong mảng lệnh `docker run`.

## 3. Hoạt động của Agent (Workspace Interaction)

1.  Agent tỉnh giấc bên trong `/workspace` của container.
2.  Agent dùng các Tools (Terminal, File System) để tạo file, ví dụ tạo file `src/index.js`.
3.  Do `/workspace` được mount từ Host, file `src/index.js` gần như lập tức được ghi vào ổ cứng thật của Host OS.
4.  Agent chạy lệnh `npm install`. Tất cả dependencies được tải về và lưu vào `node_modules/` (nằm trên Host ổ cứng, nhưng được tải bởi môi trường Container).

## 4. Kết thúc (Post-Execution teardown)

1.  Agent thoát (Exit code 0 hoặc lỗi). Tiến trình chính bên trong container sập.
2.  Do cờ `--rm`, Docker daemon dọn dẹp (xóa) container này.
3.  Orchestrator trên Host nhận được tín hiệu thoát.
4.  Orchestrator chạy hàm kiểm tra kết quả hậu kiểm (Post-Init Validation, ví dụ `npm run build`), quá trình này cũng phải được bọc trong hàm `docker run` tương tự để đảm bảo các bash command được chạy trong hệ điều hành chứa Node/Rust phù hợp.
5.  Orchestrator tiến hành lưu log, cập nhật Database và dọn dẹp.
