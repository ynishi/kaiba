-- Webhook Tables
-- Enables Rei to interact with the external world

-- ============================================
-- Rei Webhooks - Outbound webhook configuration
-- ============================================

CREATE TABLE IF NOT EXISTS rei_webhooks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rei_id UUID NOT NULL REFERENCES reis(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    secret TEXT,  -- HMAC-SHA256 signing secret
    enabled BOOLEAN NOT NULL DEFAULT true,
    events JSONB NOT NULL DEFAULT '["all"]',  -- Event types to subscribe to
    headers JSONB NOT NULL DEFAULT '{}',  -- Custom headers
    max_retries INTEGER NOT NULL DEFAULT 3,
    timeout_ms INTEGER NOT NULL DEFAULT 30000,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_rei_webhooks_rei_id ON rei_webhooks(rei_id);
CREATE INDEX IF NOT EXISTS idx_rei_webhooks_enabled ON rei_webhooks(enabled) WHERE enabled = true;

-- ============================================
-- Webhook Deliveries - Delivery tracking
-- ============================================

CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    webhook_id UUID NOT NULL REFERENCES rei_webhooks(id) ON DELETE CASCADE,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',  -- pending, success, failed, retrying
    status_code INTEGER,
    response_body TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_webhook_id ON webhook_deliveries(webhook_id);
CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_status ON webhook_deliveries(status);
CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_created_at ON webhook_deliveries(created_at DESC);

-- ============================================
-- Triggers
-- ============================================

CREATE TRIGGER update_rei_webhooks_updated_at
    BEFORE UPDATE ON rei_webhooks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
