---
name: setup-cloudflare-tunnel
description: Prepare preview tunnel for Web/API and emit machine-parseable preview target fields. Agent must log user-friendly messages when tunnel fails (see Log for User).
---

# Setup Cloudflare Tunnel

## Constraint (Required)
When this skill is active (auto_deploy enabled), you **MUST**:
1. Start the preview runtime (e.g. `docker compose up -d` or `npm run dev` in background).
2. Emit PREVIEW_TARGET before finishing—either Option A or Option B below. Without it, the preview tunnel will be skipped.
3. Only emit PREVIEW_TARGET after the local runtime is already reachable via a real HTTP check.

## Objective
Provide preview routing details that downstream pipeline can use for preview metadata. When tunnel cannot be created, **agent must output a message** for the user in the attempt log.

## Inputs
- Local runtime port for the service.
- Tunnel/domain configuration for the project.

## Workflow
1. Start the local service if not already running.
2. Confirm local service is reachable at `http://127.0.0.1:<port>` (or `localhost`).
3. Create/resolve tunnel route to the local runtime.
4. Validate route responds successfully.
5. Emit preview fields in final output (required).

## Required Output

### Option A — File Contract (Recommended, more reliable)
Ghi file `.acpms/preview-output.json` trước khi hoàn thành:

```json
{"preview_target": "http://127.0.0.1:3000", "preview_url": "https://task-xxx.example.com"}
```

Rules:
- `preview_target` luôn là local runtime URL thật, ví dụ `http://127.0.0.1:3000`
- nếu có public tunnel URL thì ghi vào `preview_url`
- nếu chưa có hoặc không tạo được public URL thì vẫn ghi `preview_url` bằng chính local URL trong `preview_target`
- không ghi contract chỉ dựa trên config/build; local URL phải đang lên thật

- Tạo thư mục `.acpms/` nếu chưa có: `mkdir -p .acpms`
- Ghi file: `echo '{"preview_target":"http://127.0.0.1:3000"}' > .acpms/preview-output.json`
- File contract được giữ lại để follow-up stop/restart còn dùng được.

### Option B — Log output (fallback)
Always print:
- `PREVIEW_TARGET: http://127.0.0.1:<port>`
Also print:
- `PREVIEW_URL: https://...` when a public URL exists
- or `PREVIEW_URL: http://127.0.0.1:<port>` when no public URL exists yet

## Log for User
**Agent must output these messages** when they occur—they appear in the attempt log (chat session).

| Condition | Message to output |
|-----------|-------------------|
| Cloudflare not configured | Cloudflare is not configured. In System Settings (/settings), ensure Account ID, API Token, Zone ID, and Base Domain are all set. |
| Tunnel creation failed | Cloudflare tunnel could not be created. In System Settings (/settings), ensure Account ID, API Token, Zone ID, and Base Domain are all set. |
| Local runtime not reachable | Local service is not reachable. Check that the dev server is running. |

## Decision Rules
| Situation | Action |
|---|---|
| Public URL not available yet | Still output `PREVIEW_TARGET`, and set `PREVIEW_URL` to the same local URL. |
| Local runtime not reachable | Output Log for User message; report root cause. **MUST** output `DEPLOYMENT_FAILURE_REASON: <explanation>`. |
| Cloudflare/tunnel failed | Output Log for User message; **MUST** output `DEPLOYMENT_FAILURE_REASON: <explanation>`. |
| Cannot provide PREVIEW_TARGET | **MUST** output `DEPLOYMENT_FAILURE_REASON: <root cause>` before finishing (e.g. app failed to start, port conflict, docker compose error, Cloudflare not configured). User needs to know why. |
| Auto-deploy Web flow | Missing `PREVIEW_TARGET` is treated as deploy metadata failure risk. |

## Guardrails
- Never output placeholder or fake URL.
- Never output PREVIEW_TARGET if the local runtime is not listening yet.
- Keep values parseable (`http://...` or `https://...`, no markdown wrappers).
- On failure: agent must output Log for User message—do not rely on backend logs.
