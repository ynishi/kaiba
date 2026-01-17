//! Kaiba CLI - Memory upload and management
//!
//! Simple CLI for interacting with Kaiba API without MCP setup.

mod api;
mod config;

use anyhow::{bail, Context, Result};
use chrono::{Duration, Utc};
use clap::{Parser, Subcommand};
use colored::Colorize;
use dialoguer::{Input, Password};
use std::collections::HashMap;
use std::fs;
use uuid::Uuid;

use api::{DocumentInput, KaibaClient};
use config::Config;

const LONG_ABOUT: &str = r#"
Kaiba CLI - AI Persona Memory Management

QUICK START:
  kaiba login                    # API認証
  kaiba profile add <name> <id>  # プロファイル追加
  kaiba memory add "content"     # メモリ追加

MEMORY SEARCH:
  kaiba memory search "query"           # 簡易検索（プレビュー表示）
  kaiba memory search "query" --full    # 全文表示

PROMPT GENERATION (推奨):
  kaiba prompt --include-memories --context "topic"

  → メモリをコンテキストに含めたプロンプトを生成
  → Claude Code / Casting など外部Tei向け
  → セマンティック検索で関連メモリを自動取得

DOCUMENT & GRAPH (GraphKai):
  kaiba doc ingest file.md       # ドキュメント取り込み
  kaiba graph rebuild            # ナレッジグラフ構築
  kaiba graph export -f dot      # Graphviz形式でエクスポート
"#;

#[derive(Parser)]
#[command(name = "kaiba")]
#[command(about = "Kaiba CLI - AI Persona Memory Management")]
#[command(long_about = LONG_ABOUT)]
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

    /// Webhook management
    Webhook {
        #[command(subcommand)]
        action: WebhookAction,
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

    /// Document operations (GraphKai source)
    Doc {
        #[command(subcommand)]
        action: DocAction,
    },

    /// Knowledge graph operations
    Graph {
        #[command(subcommand)]
        action: GraphAction,
    },
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
    /// Search memories (use 'kaiba prompt --include-memories' for semantic search)
    Search {
        /// Search query
        query: String,
        /// Max results
        #[arg(short, long, default_value = "10")]
        limit: usize,
        /// Show full content (default: 60 char preview)
        #[arg(long)]
        full: bool,
        /// Context weights for boosting/excluding topics
        /// Format: "topic:weight,topic2:weight2" (weight=0 to exclude)
        /// Example: "Rust:1.0,Finance:0"
        #[arg(short, long)]
        context: Option<String>,
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
    },
}

#[derive(Subcommand)]
enum WebhookAction {
    /// List all webhooks
    List {
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Create a new webhook
    Create {
        /// Webhook name
        #[arg(short, long)]
        name: String,
        /// Target URL
        #[arg(short, long)]
        url: String,
        /// Event types (comma-separated: learning_completed, memory_added, etc.)
        #[arg(short, long, value_delimiter = ',')]
        events: Vec<String>,
        /// Payload format (e.g., "github_issue")
        #[arg(short = 'f', long)]
        format: Option<String>,
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Update a webhook
    Update {
        /// Webhook ID
        webhook_id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New URL
        #[arg(long)]
        url: Option<String>,
        /// Enable webhook
        #[arg(long)]
        enable: bool,
        /// Disable webhook
        #[arg(long)]
        disable: bool,
        /// Event types
        #[arg(long, value_delimiter = ',')]
        events: Option<Vec<String>>,
        /// Payload format
        #[arg(short = 'f', long)]
        format: Option<String>,
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Delete a webhook
    Delete {
        /// Webhook ID
        webhook_id: String,
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Trigger a webhook (for testing)
    Trigger {
        /// Webhook ID
        webhook_id: String,
        /// Event type to simulate
        #[arg(short, long)]
        event: Option<String>,
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// List webhook deliveries
    Deliveries {
        /// Webhook ID
        webhook_id: String,
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
    },
}

#[derive(Subcommand)]
enum DocAction {
    /// List all documents
    List {
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Get a document by ID
    Get {
        /// Document ID
        doc_id: String,
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
        /// Show full content
        #[arg(long)]
        full: bool,
    },
    /// Ingest documents from files
    Ingest {
        /// Files to ingest (Markdown)
        files: Vec<String>,
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Delete documents
    Delete {
        /// Document IDs to delete
        doc_ids: Vec<String>,
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
    },
}

#[derive(Subcommand)]
enum GraphAction {
    /// Show graph statistics
    Stats {
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Rebuild knowledge graph from documents
    Rebuild {
        /// Clear existing graph before rebuild
        #[arg(long)]
        clear: bool,
        /// Only rebuild for specific document IDs (comma-separated)
        #[arg(long, value_delimiter = ',')]
        doc_ids: Vec<String>,
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Incremental rebuild (only modified documents)
    Incremental {
        /// Process documents modified in the last N hours (default: 1)
        #[arg(long, default_value = "1")]
        hours: i64,
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Export graph for visualization
    Export {
        /// Output format: json, dot
        #[arg(short, long, default_value = "json")]
        format: String,
        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<String>,
        /// Profile to use
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Get neighbors of a node
    Neighbors {
        /// Node ID
        node_id: String,
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
        Commands::Webhook { action } => cmd_webhook(action).await,
        Commands::Prompt {
            format,
            include_memories,
            context,
            profile,
            verbose,
        } => cmd_prompt(format, include_memories, context, profile, verbose).await,
        Commands::Config => cmd_config(),
        Commands::Doc { action } => cmd_doc(action).await,
        Commands::Graph { action } => cmd_graph(action).await,
    }
}

// ============================================
// Command Implementations
// ============================================

async fn cmd_login(key: Option<String>) -> Result<()> {
    let mut config = Config::load()?;

    let api_key = match key {
        Some(k) => k,
        None => Password::new()
            .with_prompt("API Key")
            .interact()
            .context("Failed to read API key")?,
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

    println!(
        "{} API key saved to {:?}",
        "✓".green(),
        Config::config_path()?
    );

    // Offer to set up a profile if none exists
    if config.profiles.is_empty() {
        println!(
            "\n{}",
            "Tip: Set up a profile to avoid typing Rei IDs:".yellow()
        );
        println!("  kaiba rei list");
        println!("  kaiba profile add mai --rei-id <REI_ID>");
        println!("  kaiba profile set mai");
    }

    Ok(())
}

async fn cmd_profile(action: ProfileAction) -> Result<()> {
    let mut config = Config::load()?;

    match action {
        ProfileAction::Add {
            name,
            rei_id,
            display_name,
        } => {
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
                println!(
                    "{} Profile '{}' added (unverified - no API key)",
                    "✓".yellow(),
                    name
                );
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
                let default_marker = if is_default {
                    " (default)".green().to_string()
                } else {
                    String::new()
                };
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
    let api_key = config
        .api_key
        .as_ref()
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
    let api_key = config
        .api_key
        .as_ref()
        .context("Not logged in. Run 'kaiba login' first.")?;

    let client = KaibaClient::new(&config.base_url, api_key);

    match action {
        MemoryAction::Add {
            content,
            file,
            r#type,
            importance,
            tags,
            profile,
        } => {
            let rei_id = config.get_rei_id(profile.as_deref())
                .context("No profile specified and no default profile set. Use -p <profile> or set a default.")?;

            // Get content from file or argument
            let memory_content = match (content, file) {
                (Some(c), None) => c,
                (None, Some(f)) => {
                    fs::read_to_string(&f).with_context(|| format!("Failed to read file: {}", f))?
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
                .add_memory(
                    &rei_id,
                    &memory_content,
                    r#type.as_deref(),
                    importance,
                    &tags,
                )
                .await?;

            let profile_name = profile
                .as_deref()
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

        MemoryAction::Search {
            query,
            limit,
            full,
            context,
            profile,
        } => {
            let rei_id = config.get_rei_id(profile.as_deref())
                .context("No profile specified and no default profile set. Use -p <profile> or set a default.")?;

            // Parse context string into HashMap
            let context_weights = parse_context_string(context.as_deref());

            let memories = client
                .search_memories(&rei_id, &query, Some(limit), context_weights)
                .await?;

            if memories.is_empty() {
                println!("No memories found for '{}'", query);
                println!(
                    "\n{}: Use '{}' for semantic search with context",
                    "Tip".cyan().bold(),
                    "kaiba prompt --include-memories --context \"topic\"".yellow()
                );
                return Ok(());
            }

            let profile_name = profile
                .as_deref()
                .or(config.default_profile.as_deref())
                .unwrap_or("default");

            println!(
                "{} results for '{}' ({}):",
                memories.len().to_string().green(),
                query,
                profile_name.cyan()
            );

            for mem in memories {
                let type_badge = format!("[{}]", mem.memory_type).dimmed();
                let date_str = mem.created_at.format("%Y-%m-%d").to_string().dimmed();
                let score_str = mem
                    .similarity
                    .map(|s| format!(" ({:.2})", s).dimmed().to_string())
                    .unwrap_or_default();

                if full {
                    println!(
                        "\n  {} {} {}{} ",
                        type_badge,
                        date_str,
                        score_str,
                        "─".repeat(40).dimmed()
                    );
                    println!("  {}", mem.content);
                } else {
                    let preview = truncate_string(&mem.content, 60);
                    println!("  {} {} {} {}", type_badge, date_str, score_str, preview);
                }
            }

            if !full {
                println!(
                    "\n{}: Use {} for full content, {} for context boost",
                    "Tip".cyan().bold(),
                    "--full".yellow(),
                    "--context \"Rust:1.0\"".yellow()
                );
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
    let api_key = config
        .api_key
        .as_ref()
        .context("Not logged in. Run 'kaiba login' first.")?;

    let rei_id = config.get_rei_id(profile.as_deref()).context(
        "No profile specified and no default profile set. Use -p <profile> or set a default.",
    )?;

    let client = KaibaClient::new(&config.base_url, api_key);

    let prompt_resp = client
        .get_prompt(&rei_id, Some(&format), include_memories, context.as_deref())
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

/// Parse context string into HashMap
/// Format: "topic:weight,topic2:weight2" (e.g., "Rust:1.0,Finance:0")
fn parse_context_string(context: Option<&str>) -> HashMap<String, f32> {
    let mut map = HashMap::new();
    if let Some(ctx) = context {
        for pair in ctx.split(',') {
            let parts: Vec<&str> = pair.trim().split(':').collect();
            if parts.len() == 2 {
                if let Ok(weight) = parts[1].trim().parse::<f32>() {
                    map.insert(parts[0].trim().to_string(), weight);
                }
            }
        }
    }
    map
}

fn cmd_config() -> Result<()> {
    let config = Config::load()?;

    println!("{}", "Configuration:".bold());
    println!("  Path: {:?}", Config::config_path()?);
    println!("  Base URL: {}", config.base_url);
    println!(
        "  API Key: {}",
        if config.api_key.is_some() {
            "Set".green()
        } else {
            "Not set".red()
        }
    );
    println!(
        "  Default Profile: {}",
        config.default_profile.as_deref().unwrap_or("None").cyan()
    );
    println!("  Profiles: {}", config.profiles.len());

    Ok(())
}

async fn cmd_webhook(action: WebhookAction) -> Result<()> {
    let config = Config::load()?;
    let api_key = config
        .api_key
        .as_ref()
        .context("Not logged in. Run 'kaiba login' first.")?;

    let client = KaibaClient::new(&config.base_url, api_key);

    match action {
        WebhookAction::List { profile } => {
            let rei_id = config.get_rei_id(profile.as_deref()).context(
                "No profile specified and no default profile set. Use -p <profile> or set a default.",
            )?;

            let webhooks = client.list_webhooks(&rei_id).await?;

            if webhooks.is_empty() {
                println!("No webhooks configured.");
                return Ok(());
            }

            let profile_name = profile
                .as_deref()
                .or(config.default_profile.as_deref())
                .unwrap_or("default");

            println!("{} ({}):", "Webhooks".bold(), profile_name.cyan());
            for webhook in webhooks {
                let status = if webhook.enabled {
                    "✓".green()
                } else {
                    "✗".red()
                };
                let format_badge = webhook
                    .payload_format
                    .as_ref()
                    .map(|f| format!(" [{}]", f).dimmed().to_string())
                    .unwrap_or_default();

                println!(
                    "  {} {} {}{}",
                    status,
                    webhook.name.cyan().bold(),
                    webhook.events.join(", ").dimmed(),
                    format_badge
                );
                println!(
                    "    {} → {}",
                    &webhook.id.to_string()[..8].dimmed(),
                    webhook.url.dimmed()
                );
            }
        }

        WebhookAction::Create {
            name,
            url,
            events,
            format,
            profile,
        } => {
            let rei_id = config.get_rei_id(profile.as_deref()).context(
                "No profile specified and no default profile set. Use -p <profile> or set a default.",
            )?;

            let webhook = client
                .create_webhook(
                    &rei_id,
                    &name,
                    &url,
                    if events.is_empty() {
                        None
                    } else {
                        Some(events)
                    },
                    format,
                )
                .await?;

            println!("{} Webhook created: {}", "✓".green(), webhook.name.cyan());
            println!("  ID: {}", webhook.id);
            println!("  URL: {}", webhook.url.dimmed());
            println!("  Events: {}", webhook.events.join(", "));
            if let Some(fmt) = webhook.payload_format {
                println!("  Format: {}", fmt.green());
            }
        }

        WebhookAction::Update {
            webhook_id,
            name,
            url,
            enable,
            disable,
            events,
            format,
            profile,
        } => {
            let rei_id = config.get_rei_id(profile.as_deref()).context(
                "No profile specified and no default profile set. Use -p <profile> or set a default.",
            )?;

            if enable && disable {
                bail!("Cannot specify both --enable and --disable");
            }

            let enabled = if enable {
                Some(true)
            } else if disable {
                Some(false)
            } else {
                None
            };

            let webhook = client
                .update_webhook(&rei_id, &webhook_id, name, url, enabled, events, format)
                .await?;

            println!("{} Webhook updated: {}", "✓".green(), webhook.name.cyan());
            println!(
                "  Status: {}",
                if webhook.enabled {
                    "Enabled".green()
                } else {
                    "Disabled".red()
                }
            );
        }

        WebhookAction::Delete {
            webhook_id,
            profile,
        } => {
            let rei_id = config.get_rei_id(profile.as_deref()).context(
                "No profile specified and no default profile set. Use -p <profile> or set a default.",
            )?;

            client.delete_webhook(&rei_id, &webhook_id).await?;

            println!("{} Webhook deleted: {}", "✓".green(), webhook_id.dimmed());
        }

        WebhookAction::Trigger {
            webhook_id,
            event,
            profile,
        } => {
            let rei_id = config.get_rei_id(profile.as_deref()).context(
                "No profile specified and no default profile set. Use -p <profile> or set a default.",
            )?;

            let delivery = client.trigger_webhook(&rei_id, &webhook_id, event).await?;

            println!(
                "{} Webhook triggered: {}",
                "✓".green(),
                delivery.event.cyan()
            );
            println!("  Delivery ID: {}", delivery.id);
            println!(
                "  Status: {}",
                match delivery.status.as_str() {
                    "success" => "Success".green(),
                    "failed" => "Failed".red(),
                    _ => delivery.status.yellow(),
                }
            );
            if let Some(code) = delivery.status_code {
                println!("  HTTP Status: {}", code);
            }
        }

        WebhookAction::Deliveries {
            webhook_id,
            profile,
        } => {
            let rei_id = config.get_rei_id(profile.as_deref()).context(
                "No profile specified and no default profile set. Use -p <profile> or set a default.",
            )?;

            let deliveries = client.list_deliveries(&rei_id, &webhook_id).await?;

            if deliveries.is_empty() {
                println!("No deliveries found.");
                return Ok(());
            }

            println!("{} deliveries:", deliveries.len().to_string().green());
            for delivery in deliveries {
                let status_badge = match delivery.status.as_str() {
                    "success" => "✓".green(),
                    "failed" => "✗".red(),
                    "pending" => "⏳".yellow(),
                    _ => "?".dimmed(),
                };

                println!(
                    "  {} {} [{}] (attempts: {})",
                    status_badge,
                    delivery.event.cyan(),
                    delivery
                        .status_code
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    delivery.attempts
                );
                println!("    {}", delivery.created_at.dimmed());
            }
        }
    }

    Ok(())
}

// ============================================
// Document Commands
// ============================================

async fn cmd_doc(action: DocAction) -> Result<()> {
    let config = Config::load()?;
    let api_key = config
        .api_key
        .as_ref()
        .context("Not logged in. Run 'kaiba login' first.")?;

    let client = KaibaClient::new(&config.base_url, api_key);

    match action {
        DocAction::List { profile } => {
            let rei_id = config.get_rei_id(profile.as_deref()).context(
                "No profile specified and no default profile set. Use -p <profile> or set a default.",
            )?;

            let docs = client.list_documents(&rei_id).await?;

            if docs.is_empty() {
                println!("No documents found.");
                return Ok(());
            }

            let profile_name = profile
                .as_deref()
                .or(config.default_profile.as_deref())
                .unwrap_or("default");

            println!("{} ({}):", "Documents".bold(), profile_name.cyan());
            for doc in docs {
                let source = doc.source_path.as_deref().unwrap_or("-").dimmed();
                println!(
                    "  {} {} {}",
                    doc.id.to_string()[..8].dimmed(),
                    doc.title.cyan().bold(),
                    source
                );
                println!(
                    "    Updated: {}",
                    doc.updated_at.format("%Y-%m-%d %H:%M").to_string().dimmed()
                );
            }
        }

        DocAction::Get {
            doc_id,
            profile,
            full,
        } => {
            let rei_id = config.get_rei_id(profile.as_deref()).context(
                "No profile specified and no default profile set. Use -p <profile> or set a default.",
            )?;

            let doc = client.get_document(&rei_id, &doc_id).await?;

            println!("{}: {}", "Title".bold(), doc.title.cyan());
            println!("{}: {}", "ID".bold(), doc.id);
            if let Some(path) = &doc.source_path {
                println!("{}: {}", "Source".bold(), path.dimmed());
            }
            println!(
                "{}: {}",
                "Created".bold(),
                doc.created_at.format("%Y-%m-%d %H:%M")
            );
            println!(
                "{}: {}",
                "Updated".bold(),
                doc.updated_at.format("%Y-%m-%d %H:%M")
            );

            if full {
                println!("\n{}", "Content:".bold());
                println!("{}", doc.raw_content);
            } else {
                println!("\n{}", "Preview:".bold());
                println!("{}", truncate_string(&doc.raw_content, 200).dimmed());
                println!("\n{}", "(use --full to see complete content)".dimmed());
            }
        }

        DocAction::Ingest { files, profile } => {
            let rei_id = config.get_rei_id(profile.as_deref()).context(
                "No profile specified and no default profile set. Use -p <profile> or set a default.",
            )?;

            if files.is_empty() {
                bail!("No files specified. Usage: kaiba doc ingest <file1> [file2] ...");
            }

            let mut documents = Vec::new();
            for file_path in &files {
                let content = fs::read_to_string(file_path)
                    .with_context(|| format!("Failed to read file: {}", file_path))?;

                // Extract title from filename or first heading
                let title = extract_title(&content, file_path);

                documents.push(DocumentInput {
                    title,
                    content,
                    source_path: Some(file_path.clone()),
                    metadata: None,
                });
            }

            let result = client.ingest_documents(&rei_id, documents).await?;

            println!(
                "{} Ingested {} documents:",
                "✓".green(),
                result.summary.total
            );
            println!(
                "  Created: {}, Updated: {}, Unchanged: {}",
                result.summary.created.to_string().green(),
                result.summary.updated.to_string().yellow(),
                result.summary.unchanged.to_string().dimmed()
            );
            println!(
                "  Emphasis nodes extracted: {}",
                result.summary.total_emphasis_nodes.to_string().cyan()
            );

            for res in &result.results {
                let status_badge = match res.status.as_str() {
                    "created" => "✓".green(),
                    "updated" => "↑".yellow(),
                    "unchanged" => "=".dimmed(),
                    _ => "?".dimmed(),
                };
                println!(
                    "  {} {} [{}]",
                    status_badge,
                    res.title.cyan(),
                    res.doc_id.to_string()[..8].dimmed()
                );
            }
        }

        DocAction::Delete { doc_ids, profile } => {
            let rei_id = config.get_rei_id(profile.as_deref()).context(
                "No profile specified and no default profile set. Use -p <profile> or set a default.",
            )?;

            if doc_ids.is_empty() {
                bail!("No document IDs specified. Usage: kaiba doc delete <doc_id1> [doc_id2] ...");
            }

            let uuids: Vec<Uuid> = doc_ids
                .iter()
                .map(|id| id.parse::<Uuid>())
                .collect::<Result<Vec<_>, _>>()
                .context("Invalid document ID format. Expected UUID.")?;

            let result = client.delete_documents(&rei_id, uuids).await?;

            println!(
                "{} Deleted {} documents",
                "✓".green(),
                result.deleted.to_string().green()
            );
            if !result.not_found.is_empty() {
                println!("  {} not found: {}", "⚠".yellow(), result.not_found.len());
            }
        }
    }

    Ok(())
}

/// Extract title from markdown content or filename
fn extract_title(content: &str, file_path: &str) -> String {
    // Try to find first heading
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            return heading.trim().to_string();
        }
    }
    // Fall back to filename without extension
    std::path::Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string()
}

// ============================================
// Graph Commands
// ============================================

async fn cmd_graph(action: GraphAction) -> Result<()> {
    let config = Config::load()?;
    let api_key = config
        .api_key
        .as_ref()
        .context("Not logged in. Run 'kaiba login' first.")?;

    let client = KaibaClient::new(&config.base_url, api_key);

    match action {
        GraphAction::Stats { profile } => {
            let rei_id = config.get_rei_id(profile.as_deref()).context(
                "No profile specified and no default profile set. Use -p <profile> or set a default.",
            )?;

            let stats = client.get_graph_stats(&rei_id).await?;

            let profile_name = profile
                .as_deref()
                .or(config.default_profile.as_deref())
                .unwrap_or("default");

            println!("{} ({}):", "Graph Statistics".bold(), profile_name.cyan());
            println!("  Total Nodes: {}", stats.total_nodes.to_string().green());
            println!("  Total Edges: {}", stats.total_edges.to_string().green());

            if !stats.nodes_by_type.is_empty() {
                println!("\n  {}:", "Nodes by Type".bold());
                for (node_type, count) in &stats.nodes_by_type {
                    println!("    {}: {}", node_type.cyan(), count);
                }
            }

            if !stats.edges_by_type.is_empty() {
                println!("\n  {}:", "Edges by Type".bold());
                for (edge_type, count) in &stats.edges_by_type {
                    println!("    {}: {}", edge_type.cyan(), count);
                }
            }
        }

        GraphAction::Rebuild {
            clear,
            doc_ids,
            profile,
        } => {
            let rei_id = config.get_rei_id(profile.as_deref()).context(
                "No profile specified and no default profile set. Use -p <profile> or set a default.",
            )?;

            let doc_id_uuids = if doc_ids.is_empty() {
                None
            } else {
                Some(
                    doc_ids
                        .iter()
                        .map(|id| id.parse::<Uuid>())
                        .collect::<Result<Vec<_>, _>>()
                        .context("Invalid document ID format. Expected UUID.")?,
                )
            };

            println!("Rebuilding graph...");
            let result = client.rebuild_graph(&rei_id, doc_id_uuids, clear).await?;

            println!("{} Graph rebuilt in {}ms:", "✓".green(), result.duration_ms);
            println!(
                "  Documents processed: {}",
                result.documents_processed.to_string().green()
            );
            println!(
                "  Nodes created: {}",
                result.nodes_created.to_string().green()
            );
            println!(
                "  Edges created: {}",
                result.edges_created.to_string().green()
            );
            println!(
                "  Nodes skipped: {}",
                result.nodes_skipped.to_string().dimmed()
            );

            if !result.errors.is_empty() {
                println!("\n  {}:", "Errors".red().bold());
                for err in &result.errors {
                    println!("    - {}", err.red());
                }
            }
        }

        GraphAction::Incremental { hours, profile } => {
            let rei_id = config.get_rei_id(profile.as_deref()).context(
                "No profile specified and no default profile set. Use -p <profile> or set a default.",
            )?;

            let since = Utc::now() - Duration::hours(hours);

            println!("Running incremental rebuild (last {} hours)...", hours);
            let result = client.incremental_rebuild(&rei_id, Some(since)).await?;

            println!(
                "{} Incremental rebuild completed in {}ms:",
                "✓".green(),
                result.duration_ms
            );
            println!(
                "  Documents found: {}",
                result.documents_found.to_string().green()
            );
            println!(
                "  Documents processed: {}",
                result.documents_processed.to_string().green()
            );
            println!(
                "  Nodes created: {}",
                result.nodes_created.to_string().green()
            );
            println!(
                "  Edges created: {}",
                result.edges_created.to_string().green()
            );
            println!(
                "  Time range: {} → {}",
                result.since.format("%Y-%m-%d %H:%M").to_string().dimmed(),
                result.until.format("%Y-%m-%d %H:%M").to_string().dimmed()
            );

            if !result.errors.is_empty() {
                println!("\n  {}:", "Errors".red().bold());
                for err in &result.errors {
                    println!("    - {}", err.red());
                }
            }
        }

        GraphAction::Export {
            format,
            output,
            profile,
        } => {
            let rei_id = config.get_rei_id(profile.as_deref()).context(
                "No profile specified and no default profile set. Use -p <profile> or set a default.",
            )?;

            let graph = client.export_graph(&rei_id).await?;

            let output_str = match format.as_str() {
                "dot" => generate_dot(&graph),
                _ => serde_json::to_string_pretty(&graph)
                    .context("Failed to serialize graph to JSON")?,
            };

            match output {
                Some(path) => {
                    fs::write(&path, &output_str)
                        .with_context(|| format!("Failed to write to {}", path))?;
                    println!(
                        "{} Graph exported to {} ({} nodes, {} edges)",
                        "✓".green(),
                        path.cyan(),
                        graph.stats.total_nodes,
                        graph.stats.total_edges
                    );
                }
                None => {
                    println!("{}", output_str);
                }
            }
        }

        GraphAction::Neighbors { node_id, profile } => {
            let rei_id = config.get_rei_id(profile.as_deref()).context(
                "No profile specified and no default profile set. Use -p <profile> or set a default.",
            )?;

            let result = client.get_node_neighbors(&rei_id, &node_id).await?;

            println!(
                "{}: {} [{}]",
                "Node".bold(),
                result.node.text.cyan().bold(),
                result.node.node_type.dimmed()
            );
            println!("  ID: {}", result.node.id);
            println!("  Weight: {:.2}", result.node.weight);

            if result.neighbors.is_empty() {
                println!("\n  No neighbors found.");
            } else {
                println!("\n  {} ({}):", "Neighbors".bold(), result.neighbors.len());
                for neighbor in &result.neighbors {
                    println!(
                        "    {} [{}] (w: {:.2})",
                        neighbor.text.cyan(),
                        neighbor.node_type.dimmed(),
                        neighbor.weight
                    );
                }
            }

            if !result.edges.is_empty() {
                println!("\n  {} ({}):", "Edges".bold(), result.edges.len());
                for edge in &result.edges {
                    let direction = if edge.from_id == result.node.id {
                        "→"
                    } else {
                        "←"
                    };
                    println!(
                        "    {} {} (strength: {:.2})",
                        direction,
                        edge.edge_type.cyan(),
                        edge.strength
                    );
                }
            }
        }
    }

    Ok(())
}

/// Generate DOT format for Graphviz
fn generate_dot(graph: &api::GraphExportResponse) -> String {
    let mut dot = String::from("digraph GraphKai {\n");
    dot.push_str("  rankdir=LR;\n");
    dot.push_str("  node [shape=box, style=rounded];\n\n");

    // Add nodes
    for node in &graph.nodes {
        let label = node.text.replace('"', "\\\"");
        let color = match node.node_type.as_str() {
            "document" => "lightblue",
            "concept" => "lightgreen",
            _ => "white",
        };
        dot.push_str(&format!(
            "  \"{}\" [label=\"{}\", fillcolor={}, style=\"filled,rounded\"];\n",
            node.id, label, color
        ));
    }

    dot.push('\n');

    // Add edges
    for edge in &graph.edges {
        let style = match edge.edge_type.as_str() {
            "extracted_from" => "dashed",
            "co_occurs_with" => "solid",
            _ => "dotted",
        };
        dot.push_str(&format!(
            "  \"{}\" -> \"{}\" [label=\"{}\", style={}];\n",
            edge.from_id, edge.to_id, edge.edge_type, style
        ));
    }

    dot.push_str("}\n");
    dot
}
