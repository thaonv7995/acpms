# GitHub Integration

Hệ thống hỗ trợ GitHub ngoài GitLab cho các thao tác VCS (fork, PR, merge).

## Crate

`crates/github/` — GitHub API client library, sử dụng Personal Access Token (PAT).

## Architecture

GitHub integration không có REST API endpoints riêng (không có `/api/v1/github/` routes). Thay vào đó, `GitHubClient` được sử dụng nội bộ bởi các services:

- **Task Attempts** (`task_attempts.rs`): Khi approve/reject attempt, tự động tạo/merge/close Pull Request trên GitHub
- **Repository Access** (`RepositoryAccessService`): Check quyền truy cập repository, tạo fork
- **GitOps** (`orchestrator-gitops.rs`): Push branches, create PRs

## GitHub Client API

**File**: `crates/github/src/client.rs`

**Initialization**:
```rust
let client = GitHubClient::new("https://github.com", "<PAT>")?;
```

**Token**: Sử dụng PAT từ `system_settings.github_pat` (cấu hình trong Settings UI).

### Methods

| Method | Mô tả |
|---|---|
| `get_authenticated_user()` | Lấy user info từ token |
| `get_repo(owner, repo)` | Lấy repo metadata + permissions |
| `create_fork(owner, repo)` | Tạo fork |
| `create_pull_request(owner, repo, params)` | Tạo PR |
| `get_pull_request(owner, repo, number)` | Lấy PR by number |
| `list_pulls_by_head(owner, repo, head)` | List open PRs by head branch |
| `merge_pull_request(owner, repo, number)` | Merge PR |
| `close_pull_request(owner, repo, number)` | Close PR without merge |

### Types

**File**: `crates/github/src/types.rs`

- `CreatePrParams`: title, head, base, body
- `PullRequest`: number, html_url, state, merged, head, base
- `GitHubUser`: login, id, name, email, avatar_url
- `GitHubRepository`: full_name, private, permissions, default_branch, fork, parent
- `MergeResult`: sha, merged, message

## Configuration

GitHub PAT được lưu trong System Settings:
- **Setting key**: `github_pat`
- **UI**: Settings Page → GitHub section
- **Storage**: Database `system_settings` table

## So sánh với GitLab

| Feature | GitLab | GitHub |
|---|---|---|
| REST API endpoints | ✅ Có (`/api/v1/gitlab/*`) | ❌ Không có riêng |
| OAuth flow | ✅ Có | ❌ Không có |
| Webhook integration | ✅ Có | ❌ Không có |
| Fork management | ✅ Có | ✅ Có (internal) |
| PR/MR operations | ✅ Có | ✅ Có (internal) |
| Auth method | OAuth2 | PAT |

## Lưu ý

GitHub integration hiện tại là **internal library** — không expose REST endpoints cho frontend. Frontend không gọi trực tiếp GitHub API mà tất cả đi qua các route `task_attempts`, `projects`, v.v.
