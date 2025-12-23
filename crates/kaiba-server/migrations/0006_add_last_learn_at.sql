-- Add last_learn_at to rei_states for dashboard tracking
-- Used to show when the last learning action was performed

ALTER TABLE rei_states
ADD COLUMN IF NOT EXISTS last_learn_at TIMESTAMPTZ;

COMMENT ON COLUMN rei_states.last_learn_at IS 'Last time Learn was completed for this Rei';
