-- Chuẩn hóa agent_cli_provider: codex -> openai-codex, gemini -> gemini-cli
-- Canonical values: claude-code | openai-codex | gemini-cli

UPDATE system_settings
SET agent_cli_provider = CASE LOWER(TRIM(agent_cli_provider))
    WHEN 'codex' THEN 'openai-codex'
    WHEN 'gemini' THEN 'gemini-cli'
    ELSE agent_cli_provider
END
WHERE LOWER(TRIM(agent_cli_provider)) IN ('codex', 'gemini');
