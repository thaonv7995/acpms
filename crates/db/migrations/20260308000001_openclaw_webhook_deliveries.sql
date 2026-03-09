CREATE TABLE IF NOT EXISTS openclaw_webhook_deliveries (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_sequence_id BIGINT NOT NULL REFERENCES openclaw_gateway_events(sequence_id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'pending', -- pending, processing, completed, failed
    attempt_count INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 5,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_status_code INTEGER,
    last_error TEXT,
    last_attempt_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(event_sequence_id)
);

CREATE INDEX IF NOT EXISTS idx_openclaw_webhook_deliveries_status
    ON openclaw_webhook_deliveries(status, next_attempt_at)
    WHERE status IN ('pending', 'failed');

CREATE INDEX IF NOT EXISTS idx_openclaw_webhook_deliveries_created
    ON openclaw_webhook_deliveries(created_at);

DROP TRIGGER IF EXISTS update_openclaw_webhook_deliveries_updated_at ON openclaw_webhook_deliveries;
CREATE TRIGGER update_openclaw_webhook_deliveries_updated_at
    BEFORE UPDATE ON openclaw_webhook_deliveries
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
