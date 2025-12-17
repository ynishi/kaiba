-- Add energy regeneration config to rei_states
-- energy_regen_per_hour: How much energy to regenerate per hour (0 = disabled)

ALTER TABLE rei_states
ADD COLUMN IF NOT EXISTS energy_regen_per_hour INTEGER NOT NULL DEFAULT 10;
