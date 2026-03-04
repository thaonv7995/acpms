# V2.0 Multi-Account: Legacy Features Migration Plan

This document outlines the current state (V1) of existing features within Agentic-Coding and the exact technical plan required to migrate them to support the **V2.0 Multi-Account Architecture**.

---

## 1. Task Executor (Orchestrator)

**Hiện trạng (V1):**
- Orchestrator (`crates/executors/src/orchestrator.rs`) chịu trách nhiệm nhận Job và gọi các client (`codex.rs`, `gemini.rs`, `claude.rs`) qua hàm `spawn_session()`.
- Hàm `spawn_session()` đã hỗ trợ tham số `env_vars: Option<HashMap<String, String>>` nhưng hiện tại Orchestrator chỉ truyền rỗng hoặc truyền một số API Keys cơ bản. Nó đang chạy CLI bằng cấu hình mặc định tải từ `~/.config` hoặc thư mục root của máy chủ.

**Kế hoạch Update (V2):**
- **Sửa ở duy nhất Orchestrator:** Trước khi gọi `spawn_session()`, Orchestrator sẽ kết nối DB lấy danh sách `agent_profiles`.
- Áp dụng thuật toán **Load Balancing (Round-Robin)** để chọn ngẫu nhiên một account đang có trạng thái `Available`.
- **Inject Biến Môi Trường:** Nhúng `config_dir` của account đó vào `env_vars`:
  - `HOME=/app/data/agent_profiles/{profile_id}`
  - `XDG_CONFIG_HOME=/app/data/agent_profiles/{profile_id}/.config`
  - `CLAUDE_SESSION_DIR=/app/data/agent_profiles/{profile_id}`
- **Giữ nguyên:** Các file lõi `codex.rs`, `gemini.rs`, `claude.rs` không cần sửa đổi bất kỳ dòng logic nào vì cơ chế map `env_vars` vào `Command::new().env()` đã được hỗ trợ sẵn.

---

## 2. API & Quản Lý Trạng Thái Auth (Agent Authentication Service)

**Hiện trạng (V1):**
- REST endpoint `POST /api/v1/agent/auth/initiate` chỉ nhận input `{ "provider": "..." }`.
- Struct `AuthSessionRecord` lưu `session_id`, `provider`, `status` nhưng không liên kết với bản ghi Account độc lập nào trong DB. Mọi trạng thái thành công (`Succeeded`) đều được coi là của toàn cục.

**Kế hoạch Update (V2):**
- Đổi contract API `initiate` để nhận thêm `{ "profile_id": "optional(uuid)" }`. Mọi yêu cầu Auth sẽ ngầm hiểu là tạo mới Account, hoặc Re-auth cho 1 Account cũ.
- Bổ sung trường `profile_id: Option<Uuid>` vào database/struct `AuthSessionRecord` (`crates/server/src/services/agent_auth.rs`).
- Khi AuthSession chuyển hướng thành `Succeeded`, Backend hook sẽ cập nhật đúng bảng `agent_profiles` của `profile_id` đó thay vì cập nhật global.

---

## 3. Worker Background: Provider Status Probing

**Hiện trạng (V1):**
- API `GET /api/v1/agent/providers/status` quét qua hàm `check_provider_status` (chạy lệnh `codex login status`, `claude auth status`...).
- Lệnh probe này chạy truy xuất thư mục global. Sẽ có 3 provider trả kết quả duy nhất cho 3 dòng trên UI.

**Kế hoạch Update (V2):**
- Sửa hàm `check_provider_status` thành `check_profile_status(config_dir: &str)`.
- Hàm Probe này phải inject biến môi trường ảo `HOME` khi chạy lệnh `-p ping` hoặc `status`.
- Hệ thống sẽ trả về danh sách trạng thái của N account (Profiles) bằng 1 API mới: `GET /api/v1/agent/profiles`.

---

## 4. Frontend UI: Settings Page

**Hiện trạng (V1):**
- Code React trong `frontend/src/pages/SettingsPage.tsx` render 3 hộp cố định, mỗi hộp 1 Provider (Codex, Claude, Gemini). Trạng thái (Available, Re-auth) được map cứng với từng Provider.
- Auth Session Modal nằm ở Right Panel hiện URL hoặc mã Code (e.g., Device Flow).

**Kế hoạch Update (V2):**
- **Left Panel (Provider List):** Sẽ đưuọc quy hoạch lại thành Expandable Accordion (Danh sách sổ xuống).
- Header mỗi hộp ghi [Codex CLI: 2 Accounts]. Bấm vào mới hiện ra danh sách các Account chi tiết. Header có nút `[+ Add Account]` để trigger `initiate`.
- **Right Panel (Auth Session):** Giữ nguyên không đổi vì Flow Authentication (cả Device Flow và PKCE) của Terminal không cần biết bạn đang chạy multi-account hay không, nó chỉ tuân thủ đúng `session_id` được trả về.

---

## 5. Metadata Storage & Database

**Hiện trạng (V1):**
- Biến cấu hình "Ai đang là AI Provider mặc định" được lưu trong bảng `projects` qua cột dạng JSONB `agent_settings`.
- Không có cấu trúc lưu danh sách nhiều Session thư mục.

**Kế hoạch Update (V2):**
- **Tạo Data Migration mới (`migrations/xxx_add_agent_profiles.sql`):**
  - Table `agent_profiles` (id, user_id, provider, profile_name, config_dir, status, created_at, updated_at).
- Khi chạy Hệ thống, `config_dir` của Profile sinh ra sẽ chốt ở `/app/data/agent_profiles/{profile_id}` để đảm bảo không một Agent task nào lấn lướt dữ liệu Token Authentication của Agent task khác (Cách ly Session tuyệt đối).

---

## Tóm Lược Rủi Ro (Risk Assessment)
1. **SSRF Proxy:** Việc quản lý nhiều localhost session song song có thể gây trễ (timeout) hoặc đụng Port Callback. Giải pháp: Random Port Generator cho Loopback.
2. **File Permission Risk:** Multiple Profile Dirs cần được cấp quyền `chmod 700` để các Worker/Agent Tasks khác không cố tình đọc trộm Auth Token của nhau.
3. **Database Race-Condition:** Orchestrator có thể chọn cùng 1 `Available` account nhiều lần cùng lúc nếu Load Balancer không có khóa `SKIP LOCKED` ở DB, dẫn tới việc bắn Request rate-limit vào 1 account Google/Anthropic. Giải pháp: Bổ sung Redis lock hoặc in-memory state tracking vòng lặp Round-Robin.
