//! Kaiba CLI - Memory upload and management
//!
//! Simple CLI for interacting with Kaiba API without MCP setup.

mod api;
mod config;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use dialoguer::{Input, Password};
use std::fs;

use api::KaibaClient;
use config::Config;

#[derive(Parser)]
#[command(name = "kaiba")]
#[command(about = "Kaiba CLI - Memory upload and management", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Login and store API key
    Login {
        /// API key (will prompt if not provided)
        #[arg(short, long)]
        key: Option<String>,
    },

    /// Manage profiles (Rei shortcuts)
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },

    /// List Reis from API
    Rei {
        #[command(subcommand)]
        action: ReiAction,
    },

    /// Memory operations
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },

    /// Get prompt for external Tei (Claude Code, Casting, etc.)
    Prompt {
        /// Output format: raw, claude-code, casting
        #[arg(short, long, default_value = "raw")]
        format: String,
        /// Include memories in prompt
        #[arg(short = 'm', long)]
        include_memories: bool,
        /// Context for memory search (defaults to Rei name)
        #[arg(short, long)]
        context: Option<String>,
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
        /// Show metadata (Rei info, memory count)
        #[arg(long)]
        verbose: bool,
    },

    /// Show current configuration
    Config,
}

#[derive(Subcommand)]
enum ProfileAction {
    /// Add a new profile
    Add {
        /// Profile name (e.g., "mai", "shii")
        name: String,
        /// Rei ID
        #[arg(long)]
        rei_id: String,
        /// Display name (optional)
        #[arg(long)]
        display_name: Option<String>,
    },
    /// List all profiles
    List,
    /// Set default profile
    Set {
        /// Profile name to set as default
        name: String,
    },
    /// Remove a profile
    Remove {
        /// Profile name to remove
        name: String,
    },
}

#[derive(Subcommand)]
enum ReiAction {
    /// List all Reis
    List,
}

#[derive(Subcommand)]
enum MemoryAction {
    /// Add a memory
    Add {
        /// Memory content (or use -f for file)
        content: Option<String>,
        /// Read content from file
        #[arg(short, long)]
        file: Option<String>,
        /// Memory type (learning, fact, expertise, reflection)
        #[arg(short = 't', long)]
        r#type: Option<String>,
        /// Importance (0.0-1.0)
        #[arg(short, long)]
        importance: Option<f32>,
        /// Tags for categorization (comma-separated, e.g., "rust,auth,orcs")
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// Profile to use (overrides default)
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Search memories
    Search {
        /// Search query
        query: String,
        /// Max results
        #[arg(short, long, default_value = "10")]
        limit: usize,
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Login { key } => cmd_login(key).await,
        Commands::Profile { action } => cmd_profile(action).await,
        Commands::Rei { action } => cmd_rei(action).await,
        Commands::Memory { action } => cmd_memory(action).await,
        Commands::Prompt { format, include_memories, context, profile, verbose } => {
            cmd_prompt(format, include_memories, context, profile, verbose).await
        }
        Commands::Config => cmd_config(),
    }
}

// ============================================
// Command Implementations
// ============================================

async fn cmd_login(key: Option<String>) -> Result<()> {
    let mut config = Config::load()?;

    let api_key = match key {
        Some(k) => k,
        None => {
            Password::new()
                .with_prompt("API Key")
                .interact()
                .context("Failed to read API key")?
        }
    };

    // Test connection
    let client = KaibaClient::new(&config.base_url, &api_key);
    print!("Testing connection... ");

    match client.health().await {
        Ok(true) => {
            println!("{}", "OK".green());
        }
        _ => {
            println!("{}", "Failed".red());
            bail!("Could not connect to Kaiba API. Check your API key.");
        }
    }

    config.set_api_key(api_key);
    config.save()?;

    println!("{} API key saved to {:?}", "✓".green(), Config::config_path()?);

    // Offer to set up a profile if none exists
    if config.profiles.is_empty() {
        println!("\n{}", "Tip: Set up a profile to avoid typing Rei IDs:".yellow());
        println!("  kaiba rei list");
        println!("  kaiba profile add mai --rei-id <REI_ID>");
        println!("  kaiba profile set mai");
    }

    Ok(())
}

async fn cmd_profile(action: ProfileAction) -> Result<()> {
    let mut config = Config::load()?;

    match action {
        ProfileAction::Add { name, rei_id, display_name } => {
            // Verify Rei exists if we have an API key
            if let Some(api_key) = &config.api_key {
                let client = KaibaClient::new(&config.base_url, api_key);
                match client.get_rei(&rei_id).await {
                    Ok(rei) => {
                        let display = display_name.clone().unwrap_or_else(|| rei.name.clone());
                        config.add_profile(name.clone(), rei_id, Some(display.clone()));
                        config.save()?;
                        println!("{} Profile '{}' added ({})", "✓".green(), name, display);
                    }
                    Err(e) => {
                        bail!("Could not verify Rei: {}", e);
                    }
                }
            } else {
                config.add_profile(name.clone(), rei_id, display_name);
                config.save()?;
                println!("{} Profile '{}' added (unverified - no API key)", "✓".yellow(), name);
            }
        }

        ProfileAction::List => {
            if config.profiles.is_empty() {
                println!("No profiles configured.");
                println!("\n{}", "Add one with:".dimmed());
                println!("  kaiba profile add <name> --rei-id <REI_ID>");
                return Ok(());
            }

            println!("{}", "Profiles:".bold());
            for (name, profile) in &config.profiles {
                let is_default = config.default_profile.as_ref() == Some(name);
                let default_marker = if is_default { " (default)".green().to_string() } else { String::new() };
                let display_name = profile.name.as_deref().unwrap_or("-");

                println!(
                    "  {} {} ({}){}",
                    name.cyan(),
                    display_name.dimmed(),
                    &profile.rei_id[..8],
                    default_marker
                );
            }
        }

        ProfileAction::Set { name } => {
            if config.set_default_profile(name.clone()) {
                config.save()?;
                println!("{} Default profile set to '{}'", "✓".green(), name);
            } else {
                bail!("Profile '{}' not found", name);
            }
        }

        ProfileAction::Remove { name } => {
            if config.remove_profile(&name) {
                // Clear default if it was the removed profile
                if config.default_profile.as_ref() == Some(&name) {
                    config.default_profile = None;
                }
                config.save()?;
                println!("{} Profile '{}' removed", "✓".green(), name);
            } else {
                bail!("Profile '{}' not found", name);
            }
        }
    }

    Ok(())
}

async fn cmd_rei(action: ReiAction) -> Result<()> {
    let config = Config::load()?;
    let api_key = config.api_key.as_ref()
        .context("Not logged in. Run 'kaiba login' first.")?;

    let client = KaibaClient::new(&config.base_url, api_key);

    match action {
        ReiAction::List => {
            let reis = client.list_reis().await?;

            if reis.is_empty() {
                println!("No Reis found.");
                return Ok(());
            }

            println!("{}", "Reis:".bold());
            for rei in reis {
                let energy_color = if rei.state.energy_level >= 50 {
                    rei.state.energy_level.to_string().green()
                } else if rei.state.energy_level >= 20 {
                    rei.state.energy_level.to_string().yellow()
                } else {
                    rei.state.energy_level.to_string().red()
                };

                println!(
                    "  {} {} [{}%] {}",
                    rei.id.to_string().dimmed(),
                    rei.name.cyan().bold(),
                    energy_color,
                    rei.role.dimmed()
                );
            }

            println!("\n{}", "Add a profile shortcut:".dimmed());
            println!("  kaiba profile add <name> --rei-id <ID>");
        }
    }

    Ok(())
}

async fn cmd_memory(action: MemoryAction) -> Result<()> {
    let config = Config::load()?;
    let api_key = config.api_key.as_ref()
        .context("Not logged in. Run 'kaiba login' first.")?;

    let client = KaibaClient::new(&config.base_url, api_key);

    match action {
        MemoryAction::Add { content, file, r#type, importance, tags, profile } => {
            let rei_id = config.get_rei_id(profile.as_deref())
                .context("No profile specified and no default profile set. Use -p <profile> or set a default.")?;

            // Get content from file or argument
            let memory_content = match (content, file) {
                (Some(c), None) => c,
                (None, Some(f)) => {
                    fs::read_to_string(&f)
                        .with_context(|| format!("Failed to read file: {}", f))?
                }
                (Some(_), Some(_)) => {
                    bail!("Cannot specify both content and --file");
                }
                (None, None) => {
                    // Interactive input
                    Input::new()
                        .with_prompt("Memory content")
                        .interact_text()
                        .context("Failed to read input")?
                }
            };

            let memory = client
                .add_memory(&rei_id, &memory_content, r#type.as_deref(), importance, &tags)
                .await?;

            let profile_name = profile.as_deref()
                .or(config.default_profile.as_deref())
                .unwrap_or("default");

            println!(
                "{} Memory added to {} [{}]",
                "✓".green(),
                profile_name.cyan(),
                memory.memory_type
            );

            // Show preview if content is long
            println!("  {}", truncate_string(&memory_content, 80).dimmed());
        }

        MemoryAction::Search { query, limit, profile } => {
            let rei_id = config.get_rei_id(profile.as_deref())
                .context("No profile specified and no default profile set. Use -p <profile> or set a default.")?;

            let memories = client
                .search_memories(&rei_id, &query, Some(limit))
                .await?;

            if memories.is_empty() {
                println!("No memories found for '{}'", query);
                return Ok(());
            }

            let profile_name = profile.as_deref()
                .or(config.default_profile.as_deref())
                .unwrap_or("default");

            println!("{} results for '{}' ({}):", memories.len().to_string().green(), query, profile_name.cyan());

            for mem in memories {
                let type_badge = format!("[{}]", mem.memory_type).dimmed();
                let preview = truncate_string(&mem.content, 60);
                println!("  {} {}", type_badge, preview);
            }
        }
    }

    Ok(())
}

async fn cmd_prompt(
    format: String,
    include_memories: bool,
    context: Option<String>,
    profile: Option<String>,
    verbose: bool,
) -> Result<()> {
    let config = Config::load()?;
    let api_key = config.api_key.as_ref()
        .context("Not logged in. Run 'kaiba login' first.")?;

    let rei_id = config.get_rei_id(profile.as_deref())
        .context("No profile specified and no default profile set. Use -p <profile> or set a default.")?;

    let client = KaibaClient::new(&config.base_url, api_key);

    let prompt_resp = client
        .get_prompt(
            &rei_id,
            Some(&format),
            include_memories,
            context.as_deref(),
        )
        .await?;

    if verbose {
        // Show metadata to stderr so stdout is clean for piping
        eprintln!(
            "{} {} ({}) [{}%] - {} format, {} memories",
            "Prompt for".dimmed(),
            prompt_resp.rei.name.cyan(),
            prompt_resp.rei.role.dimmed(),
            prompt_resp.rei.energy_level,
            prompt_resp.format.green(),
            prompt_resp.memories_included
        );
        eprintln!("{}", "---".dimmed());
    }

    // Output the prompt to stdout (clean for piping)
    println!("{}", prompt_resp.system_prompt);

    Ok(())
}

/// Truncate string safely for UTF-8 (by char count, not bytes)
fn truncate_string(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().take(max_chars).collect();
    if s.chars().count() > max_chars {
        format!("{}...", chars.into_iter().collect::<String>())
    } else {
        s.to_string()
    }
}

fn cmd_config() -> Result<()> {
    let config = Config::load()?;

    println!("{}", "Configuration:".bold());
    println!("  Path: {:?}", Config::config_path()?);
    println!("  Base URL: {}", config.base_url);
    println!(
        "  API Key: {}",
        if config.api_key.is_some() { "Set".green() } else { "Not set".red() }
    );
    println!(
        "  Default Profile: {}",
        config.default_profile.as_deref().unwrap_or("None").cyan()
    );
    println!("  Profiles: {}", config.profiles.len());

    Ok(())
}
