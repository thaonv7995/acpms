-- Add dns_record_id column to cloudflare_tunnels table
ALTER TABLE cloudflare_tunnels
ADD COLUMN dns_record_id TEXT;
