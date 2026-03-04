-- Add preferred conversation language for agent (system setting)
ALTER TABLE system_settings
ADD COLUMN IF NOT EXISTS preferred_agent_language TEXT DEFAULT 'en';

COMMENT ON COLUMN system_settings.preferred_agent_language IS 'Preferred language for agent conversation: en (English) or vi (Vietnamese). Injected into agent instructions.';
