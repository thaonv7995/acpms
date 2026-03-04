-- Add cursor-cli to agent_cli_provider options
-- Canonical values: claude-code | openai-codex | gemini-cli | cursor-cli

COMMENT ON COLUMN system_settings.agent_cli_provider IS 'Selected agent CLI provider: claude-code | openai-codex | gemini-cli | cursor-cli';

-- Normalize cursor -> cursor-cli for existing settings
UPDATE system_settings
SET agent_cli_provider = 'cursor-cli'
WHERE LOWER(TRIM(agent_cli_provider)) = 'cursor';
