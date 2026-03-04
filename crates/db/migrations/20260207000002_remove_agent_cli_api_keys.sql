-- Remove stored agent provider API keys from system_settings.
--
-- Authentication is managed manually via local CLIs on the server (login/config/env),
-- so the platform should not store provider API keys in the database.

ALTER TABLE system_settings
    DROP COLUMN IF EXISTS openai_api_key_encrypted,
    DROP COLUMN IF EXISTS gemini_api_key_encrypted;

