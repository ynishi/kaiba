-- Add payload_format column to rei_webhooks
-- Enables webhook payload transformation (e.g., "github_issue" for GitHub API)

ALTER TABLE rei_webhooks ADD COLUMN payload_format TEXT;

COMMENT ON COLUMN rei_webhooks.payload_format IS
'Payload format transformation: github_issue, slack, discord, or NULL for default Kaiba JSON';
