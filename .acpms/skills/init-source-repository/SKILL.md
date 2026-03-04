---
name: init-source-repository
description: Khởi tạo source code repository trên GitLab hoặc GitHub, push initial commit và ghi REPO_URL vào file contract.
---

# Init Source Repository

## Objective
Tạo repository source code mới trên GitLab hoặc GitHub (tùy GITLAB_URL), khởi tạo git, push commit đầu tiên và **bắt buộc** ghi REPO_URL vào file `.acpms/init-output.json` để hệ thống lưu vào database.

## Inputs
- Project name và slug.
- PAT (env: GITLAB_PAT) — dùng cho GitLab hoặc GitHub.
- Base URL (env: GITLAB_URL) — ví dụ https://gitlab.com hoặc https://github.com.
- Visibility (public/private).

## Workflow
1. Khởi tạo git repo: `git init`
2. Tạo cấu trúc cơ bản: README.md, .gitignore
3. Add và commit: `git add .` → `git commit -m "Initial commit"`
4. Add remote: `git remote add origin <repo_url>`
5. Push lên main: `git push -u origin main`
6. **Ghi file contract** `.acpms/init-output.json` (bắt buộc)

## Mandatory Output — File Contract
Sau khi push thành công, **phải** tạo file `.acpms/init-output.json` với nội dung:

```json
{"repo_url": "https://gitlab.example.com/username/project-name"}
```

Ví dụ GitLab:
```json
{"repo_url": "https://gitlab.example.com/org/project-name"}
```

Ví dụ GitHub:
```json
{"repo_url": "https://github.com/username/project-name"}
```

- Tạo thư mục `.acpms/` nếu chưa có: `mkdir -p .acpms`
- Ghi file: `echo '{"repo_url":"<url>"}' > .acpms/init-output.json`
- Hệ thống đọc file này, lưu vào `projects.repository_url`, rồi xóa file.

## Decision Rules
| Tình huống | Hành động |
|------------|-----------|
| Push thành công | Ghi `.acpms/init-output.json` ngay sau khi push. |
| Push thất bại | Dừng, báo lỗi, không ghi file. |
| Repo đã tồn tại | Dùng URL hiện có, vẫn ghi file. |

## Output Contract
- `repo_url`: URL đầy đủ của repository (trong file JSON)
- `init_status`: `success` | `failed`
- File `.acpms/init-output.json` tồn tại với `repo_url` hợp lệ (bắt buộc)
