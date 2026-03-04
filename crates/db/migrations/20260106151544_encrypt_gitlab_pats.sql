-- Migration: Encrypt existing GitLab PATs
-- Date: 2026-01-06
-- Purpose: Encrypt plaintext PATs in gitlab_configurations table
--
-- IMPORTANT: This migration requires ENCRYPTION_KEY environment variable
-- Run the companion Rust migration script to encrypt existing PATs

-- Step 1: Add comment to track encryption status
COMMENT ON COLUMN gitlab_configurations.pat_encrypted IS
'Encrypted GitLab Personal Access Token using AES-256-GCM. Must be decrypted before use.';

-- Step 2: Add index for faster lookups
CREATE INDEX IF NOT EXISTS idx_gitlab_configurations_updated_at
ON gitlab_configurations(updated_at DESC);

-- Note: Actual PAT encryption happens via Rust migration script
-- because SQL cannot access environment variables for encryption key.
-- See: crates/db/src/migrations/encrypt_pats.rs
