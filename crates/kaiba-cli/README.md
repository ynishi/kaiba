# kaiba-cli

CLI for [Kaiba](https://github.com/ynishi/kaiba) - AI persona memory management system.

## Server

This CLI requires a Kaiba server. The server implementation is available at [github.com/ynishi/kaiba](https://github.com/ynishi/kaiba).

The server is built on [Shuttle](https://shuttle.rs) and not published as a crate. To use this CLI, you'll need to deploy your own Kaiba server instance.

## Installation

```bash
cargo install kaiba-cli
```

## Usage

### Login

```bash
kaiba login
```

### Profile Management

```bash
# List Reis from API
kaiba rei list

# Add a profile (shortcut for Rei ID)
kaiba profile add shii --rei-id <REI_ID>

# Set default profile
kaiba profile set shii

# List profiles
kaiba profile list
```

### Memory Operations

```bash
# Add a memory
kaiba memory add "Learned about Rust async patterns"

# Add from file
kaiba memory add -f notes.txt

# Search memories
kaiba memory search "Rust async"
```

### Prompt Generation

Generate prompts for external Tei (Claude Code, etc.):

```bash
# Get raw prompt
kaiba prompt

# Get Claude Code format
kaiba prompt -f claude-code

# Include memories
kaiba prompt -m -f claude-code

# Use with Claude Code
claude --system-prompt "$(kaiba prompt -f claude-code)"
```

## Configuration

Config is stored at `~/.config/kaiba/config.toml`:

```toml
base_url = "https://kaiba.shuttleapp.rs"
api_key = "your-api-key"
default_profile = "shii"

[profiles.shii]
rei_id = "cd4efdf2-..."
name = "shii-chan"
```

## License

MIT
