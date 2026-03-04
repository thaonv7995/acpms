-- Add password_hash to users table
-- Created: 2026-01-07

ALTER TABLE users ADD COLUMN IF NOT EXISTS password_hash VARCHAR(255);
