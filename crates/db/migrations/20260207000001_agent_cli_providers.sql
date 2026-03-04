-- Add multi-agent CLI provider configuration to system_settings
--
-- Supports selecting which local CLI to use for agent execution:
-- - claude-code (default)
-- - openai-codex
-- - gemini-cli
--
-- Also stores optional encrypted API keys for providers that support API-key auth.

ALTER TABLE system_settings
    ADD COLUMN IF NOT EXISTS agent_cli_provider TEXT NOT NULL DEFAULT 'claude-code',
    ADD COLUMN IF NOT EXISTS openai_api_key_encrypted TEXT,
    ADD COLUMN IF NOT EXISTS gemini_api_key_encrypted TEXT;

COMMENT ON COLUMN system_settings.agent_cli_provider IS 'Selected agent CLI provider: claude-code | openai-codex | gemini-cli';
COMMENT ON COLUMN system_settings.openai_api_key_encrypted IS 'AES-256-GCM encrypted OpenAI API key (optional; used for Codex CLI auth)';
COMMENT ON COLUMN system_settings.gemini_api_key_encrypted IS 'AES-256-GCM encrypted Gemini API key (optional; used for Gemini CLI auth)';

