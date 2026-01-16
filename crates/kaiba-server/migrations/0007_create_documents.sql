-- Documents table for GraphKai (Source of Truth)
-- Raw Markdown storage for graph reconstruction

CREATE TABLE documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rei_id UUID NOT NULL REFERENCES reis(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    raw_content TEXT NOT NULL,
    source_path TEXT,
    checksum TEXT NOT NULL,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- Prevent duplicate documents per rei
    UNIQUE(rei_id, checksum)
);

-- Indexes for common queries
CREATE INDEX idx_documents_rei_id ON documents(rei_id);
CREATE INDEX idx_documents_checksum ON documents(checksum);
CREATE INDEX idx_documents_updated_at ON documents(updated_at);
CREATE INDEX idx_documents_created_at ON documents(created_at);

-- Trigger for auto-updating updated_at
CREATE OR REPLACE FUNCTION update_documents_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER documents_updated_at_trigger
    BEFORE UPDATE ON documents
    FOR EACH ROW
    EXECUTE FUNCTION update_documents_updated_at();
