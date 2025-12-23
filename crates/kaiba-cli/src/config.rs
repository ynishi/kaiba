//! Configuration management for Kaiba CLI
//!
//! Stores API key, profiles, and default settings in ~/.config/kaiba/config.toml

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const CONFIG_DIR: &str = "kaiba";
const CONFIG_FILE: &str = "config.toml";

/// Profile for a Rei
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub rei_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// CLI Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_profile: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

fn default_base_url() -> String {
    "https://kaiba-zlje.shuttle.app".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: default_base_url(),
            default_profile: None,
            profiles: HashMap::new(),
        }
    }
}

impl Config {
    /// Get the config directory path
    pub fn config_dir() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?
            .join(CONFIG_DIR);
        Ok(config_dir)
    }

    /// Get the config file path
    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join(CONFIG_FILE))
    }

    /// Load config from file, or create default
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config from {:?}", path))?;

        let config: Config =
            toml::from_str(&content).with_context(|| "Failed to parse config file")?;

        Ok(config)
    }

    /// Save config to file
    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir()?;
        fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create config directory {:?}", dir))?;

        let path = Self::config_path()?;
        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(&path, content)
            .with_context(|| format!("Failed to write config to {:?}", path))?;

        Ok(())
    }

    /// Set API key
    pub fn set_api_key(&mut self, key: String) {
        self.api_key = Some(key);
    }

    /// Add a profile
    pub fn add_profile(&mut self, name: String, rei_id: String, display_name: Option<String>) {
        self.profiles.insert(
            name,
            Profile {
                rei_id,
                name: display_name,
            },
        );
    }

    /// Remove a profile
    pub fn remove_profile(&mut self, name: &str) -> bool {
        self.profiles.remove(name).is_some()
    }

    /// Set default profile
    pub fn set_default_profile(&mut self, name: String) -> bool {
        if self.profiles.contains_key(&name) {
            self.default_profile = Some(name);
            true
        } else {
            false
        }
    }

    /// Get the active profile (specified or default)
    pub fn get_profile(&self, name: Option<&str>) -> Option<&Profile> {
        let profile_name = name
            .map(|s| s.to_string())
            .or_else(|| self.default_profile.clone())?;

        self.profiles.get(&profile_name)
    }

    /// Get Rei ID from profile
    pub fn get_rei_id(&self, profile: Option<&str>) -> Option<String> {
        self.get_profile(profile).map(|p| p.rei_id.clone())
    }
}
