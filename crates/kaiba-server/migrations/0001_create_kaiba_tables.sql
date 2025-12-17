-- Kaiba Database Schema
-- Rei (霊): Persistent Identity
-- Tei (体): Execution Interface

-- ============================================
-- Rei (霊) - Persona Identity
-- ============================================

CREATE TABLE IF NOT EXISTS reis (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    role TEXT NOT NULL,
    avatar_url TEXT,
    manifest JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Rei State
CREATE TABLE IF NOT EXISTS rei_states (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rei_id UUID NOT NULL REFERENCES reis(id) ON DELETE CASCADE,
    token_budget INTEGER NOT NULL DEFAULT 100000,
    tokens_used INTEGER NOT NULL DEFAULT 0,
    energy_level INTEGER NOT NULL DEFAULT 100 CHECK (energy_level >= 0 AND energy_level <= 100),
    mood TEXT NOT NULL DEFAULT 'neutral',
    last_active_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(rei_id)
);

CREATE INDEX IF NOT EXISTS idx_rei_states_rei_id ON rei_states(rei_id);

-- ============================================
-- Tei (体) - Execution Interface
-- ============================================

CREATE TABLE IF NOT EXISTS teis (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    provider TEXT NOT NULL,  -- anthropic, openai, google
    model_id TEXT NOT NULL,  -- claude-3-5-sonnet, gpt-4, gemini-pro
    is_fallback BOOLEAN NOT NULL DEFAULT false,
    priority INTEGER NOT NULL DEFAULT 0,
    config JSONB NOT NULL DEFAULT '{}',
    expertise JSONB,  -- Serialized llm-toolkit Expertise
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================
-- Rei-Tei Association (Many-to-Many)
-- ============================================

CREATE TABLE IF NOT EXISTS rei_teis (
    rei_id UUID NOT NULL REFERENCES reis(id) ON DELETE CASCADE,
    tei_id UUID NOT NULL REFERENCES teis(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (rei_id, tei_id)
);

CREATE INDEX IF NOT EXISTS idx_rei_teis_rei_id ON rei_teis(rei_id);
CREATE INDEX IF NOT EXISTS idx_rei_teis_tei_id ON rei_teis(tei_id);

-- ============================================
-- Call Logs
-- ============================================

CREATE TABLE IF NOT EXISTS call_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rei_id UUID NOT NULL REFERENCES reis(id) ON DELETE CASCADE,
    tei_id UUID NOT NULL REFERENCES teis(id) ON DELETE CASCADE,
    message TEXT NOT NULL,
    response TEXT NOT NULL,
    tokens_consumed INTEGER NOT NULL DEFAULT 0,
    context JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_call_logs_rei_id ON call_logs(rei_id);
CREATE INDEX IF NOT EXISTS idx_call_logs_created_at ON call_logs(created_at DESC);

-- ============================================
-- Triggers for updated_at
-- ============================================

CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_reis_updated_at
    BEFORE UPDATE ON reis
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_rei_states_updated_at
    BEFORE UPDATE ON rei_states
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_teis_updated_at
    BEFORE UPDATE ON teis
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
