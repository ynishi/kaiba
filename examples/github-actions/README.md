# GitHub Actions Examples

This directory contains example GitHub Actions workflows for Kaiba deployments.

## trigger.yml

Periodically triggers Kaiba's autonomous jobs to:
- Prevent serverless platform (e.g., Shuttle.rs) from going idle/sleep
- Ensure regular energy regeneration for Reis
- Execute Learn/Digest cycles on schedule

### Usage

1. Copy `trigger.yml` to your project's `.github/workflows/` directory
2. Set up GitHub Secrets:
   - `KAIBA_URL`: Your Kaiba deployment URL (e.g., `https://your-app.shuttle.app`)
   - `KAIBA_API_KEY`: Your Kaiba API key for authentication
3. Adjust the cron schedule if needed (default: every 2 hours)

### Configuration

```yaml
schedule:
  # Every 2 hours at minute 0
  - cron: '0 */2 * * *'
```

You can also trigger manually via GitHub Actions UI using `workflow_dispatch`.

### Requirements

- Kaiba server deployed and accessible
- Valid API key configured in GitHub Secrets
- `/kaiba/trigger` endpoint available
