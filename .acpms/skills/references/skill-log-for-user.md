# Skill: Log for User

> When creating or updating skills with failure/skip paths, include a **Log for User** section.

## Purpose
Attempt log is a **chat session** with the agent. When something fails or is skipped, the **agent** (not the backend) should output a clear message so the user knows what happened. Skills must describe these messages explicitly so the agent can self-log—reducing hardcoded backend logs.

## When to Include
Include a **Log for User** section when the skill has:
- Failure paths (config missing, API error, validation failed)
- Skip paths (deployment skipped, feature not available)
- Recovery hints (user can fix by doing X)

## Format
```markdown
## Log for User
When [condition], output this message (appears in attempt log):

| Condition | Message |
|-----------|---------|
| Config missing | "Cloudflare is not configured. Configure in System Settings (/settings) to enable preview. Task completed successfully." |
| Tunnel failed | "Cloudflare tunnel could not be created. Check Settings. Task completed successfully." |
```

## Principles
- **Conversational**: Like the agent speaking to the user, not system logs
- **Actionable**: Tell user what they can do (e.g. "Configure in Settings")
- **Non-alarming**: On non-critical failures, reassure (e.g. "Task completed successfully")
- **No jargon**: Avoid "deployment finalization", "worktree", "GitOps"

## Example (Cloudflare)
When Cloudflare config is missing or tunnel creation fails:
- ❌ "Pre-success deployment/report hook failed"
- ❌ "DEPLOY_PRECHECK=skipped_cloudflare_not_configured"
- ✅ "Cloudflare is not configured. Configure in System Settings (/settings) to enable preview. Task completed successfully."
