# Global Knowledge Base & SKILLs RAG: 02 - Retrieval Architecture

## 1. Cơ chế Hoạt động (How it works)

Làm thế nào để Agent tìm đúng SKILL cần thiết giữa hàng ngàn file Markdown được tải về từ GitHub / Internet? Câu trả lời là **RAG (Retrieval-Augmented Generation)**.

Quy trình sẽ không nhồi nhét toàn bộ Knowledge Base vào System Prompt (sẽ bị tràn token và AI bị nhiễu). Thay vào đó, Orchestrator sẽ kết nối Agent với kho tri thức một cách thông minh:

### Bước 1: Ingestion & Vectorization (Tiêu hóa và nhúng)
1. Orchestrator quét (scan) tất cả các file SKILL (`.md`, `.yaml`) đang có trong thư mục trung tâm (ví dụ: `~/.acpms/knowledge/skills/`).
2. Orchestrator có thể dùng một local embedding model nhỏ gọn (như *all-MiniLM-L6-v2*) để biến nội dung các SKILL này thành các vector toán học, rồi lưu vào một Vector Database cục bộ siêu nhẹ (ví dụ hệ CSDL vector dùng SQLite - `sqlite-vec` hoặc Qdrant).
3. Nếu không muốn quá phức tạp (với Vector DB), hệ thống có thể dùng **Full-text Search** (vd: Meilisearch) hoặc đơn giản là `grep` vào Tags/Tên SKILL do người dùng gán.

### Bước 2: Retrieval (Agent tự tìm kiếm)
Agent được cung cấp một bộ Tool (MCP) đặc biệt để giao tiếp với Knowledge Base:
*   `search_skills(query: string) -> List[SkillMeta]`
*   `read_skill(skill_id: string) -> MarkdownContent`

**Luồng thực thi của Agent:**
- Giả sử User yêu cầu: *"Viết giao diện login dùng Tailwind v4"*.
- Agent tự suy luận: *"Tailwind v4 mới ra mắt, có lẽ code của mình bị out-date. Mình cần xem có SKILL nào cho Tailwind v4 không"*.
- Agent gọi Tool: `search_skills(query: "tailwind v4 styling")`.
- Orchestrator (Backend) tìm trong kho dữ liệu (những SKILL đã sync từ GitHub về) và trả lại: `["tailwind_v4_migration_guide.md", "acme_corp_button_standard.md"]`.
- Agent gọi Tool `read_skill()` để đọc nội dung file đó.

### Bước 3: Augmented Generation (Sinh Code có định hướng)
Sau khi đọc được SKILL, Agent "giác ngộ" và dùng context từ file Markdown đó trộn với yêu cầu hiện tại để đẻ ra mã nguồn (Code Generation) chuẩn xác 100% theo đúng convention/tài liệu mới nhất.

## 2. Kiến trúc Tổ chức Thư mục (Directory Layout)

Cấu trúc trên máy Host sẽ tổ chức tách biệt theo "Gói" (Packages/Repositories) để dễ dàng quản lý phiên bản và cập nhật.

```text
~/.acpms/knowledge/
├── internal/                          # SKILLs riêng do công ty/cá nhân tự viết
│   ├── team_conventions/
│   │   ├── react_state.md
│   │   └── api_error_handling.md
├── remote/                            # SKILLs kéo từ Internet/Github
│   ├── official-docs/                 # Tự động parse từ scraping
│   │   ├── nextjs-15-app-router.md
│   │   └── supabase-rust-sdk.md
│   ├── github-community/              # Clone nguyên 1 repo github cộng đồng
│   │   └── agentic-coding-skills-repo/
│   │       ├── tailwind.md
│   │       ├── auth_patterns.md
│   │       └── ...
```

## 3. Worker Background (Cron) cho Global Sync
Để giữ kiến thức luôn tươi mới:
- Orchestrator cần có một cron job nhỏ chạy ngầm.
- Mỗi ngày 1 lần (hoặc có nút ấn tay trên UI Dashboard), Orchestrator thực thi lệnh `git pull` để lấy cập nhật mới nhất từ các repo SKILL cộng đồng trên GitHub.
- Orchestrator quét lại các file Markdown bị thay đổi / tạo mới, và re-index lại vào bộ tìm kiếm cục bộ.
