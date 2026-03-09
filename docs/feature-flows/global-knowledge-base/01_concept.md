# Global Knowledge Base & SKILLs RAG: 01 - Concept

## 1. Vấn đề (The Problem)

AI Agents (như Claude, Cursor) mang trong mình lượng kiến thức lập trình khổng lồ từ dữ liệu huấn luyện (Pre-trained Data). Tuy nhiên, kiến thức này mang tính "đại trà" (generic) và "cũ" (chỉ đến thời điểm model được huấn luyện).
Khi áp dụng vào thực tế dự án của người dùng hoặc công ty, hệ thống gặp các lỗi sau:
1.  **Lỗi Coding Convention**: Code chạy được nhưng không đúng chuẩn mực của team (ví dụ: Team quản lý State bằng Zustand nhưng AI lại viết Redux).
2.  **Khủng hoảng Framework mới**: Khi Framework ra mắt phiên bản mới với Breaking Changes (ví dụ: Next.js App Router thay vì Pages Router, React 19 API mới), AI thường viết code theo chuẩn cũ rích dẫn đến lỗi biên dịch.
3.  **Thiếu Best Practices**: Cùng một logic, có nhiều cách viết. Lập trình viên lão làng đúc kết ra những "SKILL" (mẹo, pattern chuẩn), nhưng Agent thì tự mò mẫm làm theo bản năng.

## 2. Ý tưởng Cốt lõi (The Vision)

Xây dựng một **Global Knowledge Base (Kho tàng tri thức toàn cục)** dành riêng cho Agent, tách biệt khỏi mã nguồn dự án. Hệ thống này hoạt động như một bộ não mở rộng để Agent vay mượn kiến thức trước khi bắt đầu code.

### Bản chất của Knowledge Base
Hệ thống KHÔNG bắt Agent đọc lại toàn bộ Internet, mà thay vào đó, hệ thống sẽ đi nhặt, gom, và lập chỉ mục (index) các mảnh kiến thức được tinh lọc gọi là **SKILLs**.

Một **SKILL** là một file tóm lược (Markdown, JSON, hoặc YAML) chứa:
- Tên SKILL (vd: `nextjs_app_router_fetching`).
- Đoạn mô tả ngữ cảnh sử dụng (vd: "Luôn dùng fetch API chuẩn trên Server Components thay vì getServerSideProps").
- Code ví dụ mẫu (Mẫu đúng / Mẫu sai).

### Nguồn cung cấp SKILLs
Kho dữ liệu này không tự sinh ra mà được "kéo về" (sync/pull) từ các nguồn chất lượng cao:
1.  **Từ Internet / Cộng đồng Open-Source (GitHub):** 
    Cộng đồng lập trình viên thế giới có thể đóng góp vào một kho lưu trữ `.github/agent-skills` công khai. Agentic-Coding có thể cung cấp tính năng "Subscribe" (đăng ký) để hệ thống tự động `git pull` các SKILL xịn nhất của cộng đồng về máy chủ cục bộ.
2.  **Từ Tài liệu Framework:** 
    Tự động cào (scraping/crawling) các trang web Official Docs (như trang của TailwindCSS, Supabase) và chuyển đổi thành file Markdown SKILL.
3.  **Từ Nội bộ Doanh nghiệp/Team:** 
    Trưởng nhóm (Tech Lead) viết tay các file `company_convention.md` hoặc `api_guidelines.md` thả vào thư mục Knowledge Base. Agent của mọi thành viên trong công ty sẽ dùng chung một bộ não này.
