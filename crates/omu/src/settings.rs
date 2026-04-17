use crate::error::CliError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) auth: Option<AuthConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AuthConfig {
    #[serde(default = "default_auth_provider")]
    pub(crate) provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) key_id_env: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) private_key_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) private_key_env: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) updated_at: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            provider: default_auth_provider(),
            key_id: None,
            key_id_env: None,
            private_key_path: None,
            private_key_env: None,
            created_at: None,
            updated_at: None,
        }
    }
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

pub(crate) fn redacted_config_value(config: &OmuConfig) -> Value {
    let profiles = config
        .profiles
        .iter()
        .map(|(name, profile)| (name.clone(), redacted_profile_value(profile)))
        .collect::<serde_json::Map<_, _>>();

    json!({
        "default_profile": config.default_profile,
        "profiles": profiles,
    })
}

pub(crate) fn redacted_profile_value(profile: &ProfileConfig) -> Value {
    json!({
        "daemon_url": profile.daemon_url,
        "kalshi_config": profile.kalshi_config,
        "policy": profile.policy,
        "auth": redacted_auth_value(profile.auth.as_ref()),
    })
}

pub(crate) fn redacted_auth_value(auth: Option<&AuthConfig>) -> Value {
    let Some(auth) = auth else {
        return Value::Null;
    };

    json!({
        "provider": auth.provider,
        "key_id_configured": auth.key_id.is_some() || auth.key_id_env.is_some(),
        "key_id": auth.key_id.as_deref().map(redact_identifier),
        "key_id_env": auth.key_id_env,
        "private_key_configured": auth.private_key_path.is_some() || auth.private_key_env.is_some(),
        "private_key_path": auth.private_key_path,
        "private_key_env": auth.private_key_env,
        "created_at": auth.created_at,
        "updated_at": auth.updated_at,
    })
}

pub(crate) fn redact_identifier(value: &str) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    let len = chars.len();
    if len <= 4 {
        return "****".to_string();
    }
    if len <= 8 {
        return format!("{}…{}", chars[0], chars[len - 1]);
    }

    let start = chars.iter().take(4).collect::<String>();
    let end = chars.iter().skip(len.saturating_sub(4)).collect::<String>();
    format!("{start}…{end}")
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

fn default_auth_provider() -> String {
    "kalshi".to_string()
}
