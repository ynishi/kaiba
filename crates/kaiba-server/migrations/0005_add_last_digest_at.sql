-- Add last_digest_at to rei_states for tracking digest completion
-- Used to filter already-digested learning memories

ALTER TABLE rei_states
ADD COLUMN IF NOT EXISTS last_digest_at TIMESTAMPTZ;

COMMENT ON COLUMN rei_states.last_digest_at IS 'Last time Digest was completed for this Rei';
