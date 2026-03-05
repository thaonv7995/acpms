# Ephemeral Sandbox: 01 - Overview & Architecture

## 1. Vấn đề hiện tại (The Problem)

Trong kiến trúc của `Agentic-Coding`, tính năng cốt lõi là việc Agent (`Claude Code`, `OpenAI Codex`, `Gemini CLI`, `Cursor CLI`) được cấp quyền thực thi các lệnh bash/shell trực tiếp trên máy chủ lưu trữ dự án (Host OS).
Mặc dù tiện lợi, phương pháp này tiềm ẩn rủi ro rất lớn về **Bảo mật và Độ ổn định**:
*   **Destructive Commands**: Agent có thể lỡ tay chạy `rm -rf /` hoặc chỉnh sửa các thư mục hệ thống bên ngoài dự án.
*   **Dependency Pollution**: Việc cài đặt các package (`npm install`, `pip install -g`) trên máy chủ chung có thể gây xung đột phiên bản giữa các projects khác nhau.
*   **Malicious Code Execution**: Một bài toán hoặc issue từ bên ngoài có thể chứa mã độc (Poisoning), dẫn đến việc Server bị chiếm quyền điều khiển.

## 2. Giải pháp: Ephemeral Sandbox (Môi trường Sandbox tạm thời)

Để giải quyết vấn đề trên, hệ thống cần đưa Agent vào chạy trong một môi trường bị cô lập hoàn toàn (Isolated Environment). Các môi trường này được khởi tạo tức thời (Ephemeral) ngay trước khi phiên chạy bắt đầu, và tự động bị hủy (Destroyed) ngay khi Agent hoàn thành công việc.

Có 2 công nghệ chính được xem xét cho quá trình này:
1.  **Docker Containers (Khuyến nghị cho v1)**: Sử dụng các Docker Image được dựng sẵn. Nhanh, nhẹ, dễ cài đặt cho người dùng tự host.
2.  **Firecracker MicroVMs (Sử dụng cho nền tảng Cloud Scale)**: Tạo máy ảo cực nhẹ với kernel riêng biệt. Cách điện hoàn toàn mã độc khỏi Host kernel (tương tự cách AWS Lambda, E2B hoạt động). Cần cấu hình KVM phức tạp hơn.

Trong tài liệu thiết kế này, chúng ta sẽ tập trung vào phương pháp **Docker Container + Volume Mount**.

## 3. Kiến trúc Docker + Volume Mount

Khi giam Agent vào trong một Docker container (không có đặc quyền, non-root), Agent sẽ mất quyền truy cập trực tiếp vào ổ cứng thật của hệ thống. Để Agent vẫn viết code được cho Project, hệ thống Orchestrator sẽ kết hợp **Volume Mounting (Bind Mount)**.

### Môi trường thực thi của Agent:
*   **Base Image**: Một Docker Image chứa đầy đủ các công cụ dev thông dụng (Node.js, Rust `cargo`, Python, Go, Git, jq...) và CLI của các Agent (Claude, Cursor...).
*   **Mount Point**: Orchestrator (chạy trên Host) sẽ mở một "lỗ hổng" nhỏ có giới hạn. Nó "mount" thư mục project hiện tại (ví dụ: `/var/acpms/worktrees/project-123`) vào bên trong container tại `/workspace`.
*   **User Mapping**: Container sẽ chạy với `UID` và `GID` tương ứng với quyền của user trên máy Host, đảm bảo file tạo ra từ container không bị khóa quyền (root-owned) khi Orchestrator xử lý bên ngoài.
*   **Resource Limits**: Container bị giới hạn chặt chẽ về CPU, RAM, và (tùy chọn) là cả Network (không cho phép kết nối tới những private IP khác trên cùng máy chủ).
