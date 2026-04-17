use crate::error::CliError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(crate) const DEFAULT_PROFILE: &str = "default";
pub(crate) const DEFAULT_DAEMON_URL: &str = "http://127.0.0.1:3030";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OmuConfig {
    #[serde(default = "default_profile_name")]
    pub(crate) default_profile: String,
    #[serde(default)]
    pub(crate) profiles: BTreeMap<String, ProfileConfig>,
}

impl Default for OmuConfig {
    fn default() -> Self {
        Self {
            default_profile: DEFAULT_PROFILE.to_string(),
            profiles: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ProfileConfig {
    pub(crate) daemon_url: Option<String>,
    pub(crate) kalshi_config: Option<String>,
    #[serde(default)]
    pub(crate) policy: PolicyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PolicyConfig {
    #[serde(default)]
    pub(crate) dry_run_default: bool,
    #[serde(default)]
    pub(crate) allow_live: bool,
    pub(crate) max_position_usd: Option<f64>,
    pub(crate) max_bankroll_usd: Option<f64>,
    #[serde(default = "default_require_yes")]
    pub(crate) require_yes: bool,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            dry_run_default: false,
            allow_live: false,
            max_position_usd: None,
            max_bankroll_usd: None,
            require_yes: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LoadedSettings {
    pub(crate) path: PathBuf,
    pub(crate) exists: bool,
    pub(crate) config: OmuConfig,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResolvedProfile {
    pub(crate) name: String,
    pub(crate) exists: bool,
    pub(crate) daemon_url: String,
    pub(crate) kalshi_config: Option<String>,
    pub(crate) dry_run_default: bool,
    pub(crate) policy: PolicyConfig,
}

impl LoadedSettings {
    pub(crate) fn load(config_dir: Option<&Path>) -> Result<Self, CliError> {
        let path = config_path(config_dir)?;
        if !path.exists() {
            return Ok(Self {
                path,
                exists: false,
                config: OmuConfig::default(),
            });
        }

        let content = std::fs::read_to_string(&path)?;
        let config = toml::from_str(&content)?;
        Ok(Self {
            path,
            exists: true,
            config,
        })
    }

    pub(crate) fn save(&mut self) -> Result<(), CliError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(&self.config)?;
        std::fs::write(&self.path, content)?;
        self.exists = true;
        Ok(())
    }

    pub(crate) fn resolve(&self, requested_profile: Option<&str>) -> ResolvedProfile {
        let name = requested_profile
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(&self.config.default_profile)
            .to_string();
        let profile = self.config.profiles.get(&name);
        let policy = profile.map(|p| p.policy.clone()).unwrap_or_default();
        ResolvedProfile {
            name,
            exists: profile.is_some(),
            daemon_url: profile
                .and_then(|p| p.daemon_url.clone())
                .unwrap_or_else(|| DEFAULT_DAEMON_URL.to_string()),
            kalshi_config: profile.and_then(|p| p.kalshi_config.clone()),
            dry_run_default: policy.dry_run_default,
            policy,
        }
    }
}

pub(crate) fn config_path(config_dir: Option<&Path>) -> Result<PathBuf, CliError> {
    if let Some(config_dir) = config_dir {
        return Ok(config_dir.join("omu.toml"));
    }
    if let Ok(dir) = std::env::var("OMU_CONFIG_DIR") {
        return Ok(PathBuf::from(dir).join("omu.toml"));
    }
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(dir).join("openmarketui/omu.toml"));
    }
    if let Ok(home) = std::env::var("HOME") {
        return Ok(PathBuf::from(home).join(".config/openmarketui/omu.toml"));
    }
    Ok(std::env::current_dir()?.join("omu.toml"))
}

fn default_profile_name() -> String {
    DEFAULT_PROFILE.to_string()
}

fn default_require_yes() -> bool {
    true
}
