use super::{audit_event, require_yes};
use crate::cli::{AuthCommand, AuthCredentialsArgs, AuthSubcommand, Cli};
use crate::client::DaemonClient;
use crate::error::CliError;
use crate::output::OutputContext;
use crate::settings::{
    redacted_auth_value, AuthConfig, LoadedSettings, ProfileConfig, DEFAULT_DAEMON_URL,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::path::Path;

pub(super) async fn handle(
    cli: &Cli,
    context: &OutputContext,
    client: &DaemonClient,
    command: &AuthCommand,
) -> Result<Value, CliError> {
    match &command.command {
        AuthSubcommand::Status(args) => status(cli, context, client, args.local_only).await,
        AuthSubcommand::Add(args) => add(cli, context, client, &args.credentials).await,
        AuthSubcommand::Rotate(args) => rotate(cli, context, client, &args.credentials).await,
    }
}

async fn status(
    cli: &Cli,
    context: &OutputContext,
    client: &DaemonClient,
    local_only: bool,
) -> Result<Value, CliError> {
    let settings = LoadedSettings::load(cli.config_dir.as_deref())?;
    let profile = settings.config.profiles.get(&context.profile);
    let auth = profile.and_then(|profile| profile.auth.as_ref());
    let local = local_auth_status(&context.profile, auth);

    if local_only {
        return Ok(json!({
            "profile": context.profile,
            "config_path": settings.path.display().to_string(),
            "local": local,
            "daemon_checked": false,
            "ready_for_live": false,
            "live_blocked_reason": "live trading remains blocked until daemon live auth gates are implemented",
        }));
    }

    let daemon = match client.get_value("/api/auth/status").await {
        Ok(value) => json!({
            "reachable": true,
            "status": value,
        }),
        Err(err) => json!({
            "reachable": false,
            "error": {
                "code": err.code(),
                "message": err.to_string(),
                "hint": err.hint(),
            }
        }),
    };

    let local_available = local
        .get("available")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let daemon_live_supported = daemon
        .get("status")
        .and_then(|status| status.get("live_trading_supported"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(json!({
        "profile": context.profile,
        "config_path": settings.path.display().to_string(),
        "local": local,
        "daemon": daemon,
        "daemon_checked": true,
        "ready_for_live": local_available && daemon_live_supported,
        "live_blocked_reason": if daemon_live_supported {
            "local credentials are not fully available"
        } else {
            "daemon reports live trading/auth gates are not implemented yet"
        },
    }))
}

async fn add(
    cli: &Cli,
    context: &OutputContext,
    client: &DaemonClient,
    credentials: &AuthCredentialsArgs,
) -> Result<Value, CliError> {
    validate_credentials(credentials)?;
    let mut settings = LoadedSettings::load(cli.config_dir.as_deref())?;
    let next = build_added_auth(credentials)?;
    let profile_existed = settings.config.profiles.contains_key(&context.profile);

    if cli.dry_run {
        return Ok(json!({
            "dry_run": true,
            "would": {
                "action": if profile_existed { "add auth to profile" } else { "create profile and add auth" },
                "path": settings.path.display().to_string(),
                "profile": context.profile,
                "auth": redacted_auth_value(Some(&next)),
            },
            "status": local_auth_status(&context.profile, Some(&next)),
        }));
    }

    require_yes(cli, "adding auth credentials")?;
    upsert_profile(&mut settings, context).auth = Some(next.clone());
    settings.save()?;

    let mut result = json!({
        "updated": true,
        "created_profile": !profile_existed,
        "path": settings.path.display().to_string(),
        "profile": context.profile,
        "auth": redacted_auth_value(Some(&next)),
        "status": local_auth_status(&context.profile, Some(&next)),
    });
    attach_auth_audit(client, cli, "auth.add", &next, &mut result).await;
    Ok(result)
}

async fn rotate(
    cli: &Cli,
    context: &OutputContext,
    client: &DaemonClient,
    credentials: &AuthCredentialsArgs,
) -> Result<Value, CliError> {
    validate_credentials(credentials)?;
    if !credentials_have_any_change(credentials) {
        return Err(CliError::InvalidArgument {
            message: "auth rotate requires at least one credential source argument".to_string(),
        });
    }

    let mut settings = LoadedSettings::load(cli.config_dir.as_deref())?;
    let existing = settings
        .config
        .profiles
        .get(&context.profile)
        .and_then(|profile| profile.auth.clone())
        .ok_or_else(|| CliError::NotFound {
            resource: "auth profile".to_string(),
            id: context.profile.clone(),
        })?;
    let next = build_rotated_auth(&existing, credentials)?;

    if cli.dry_run {
        return Ok(json!({
            "dry_run": true,
            "would": {
                "action": "rotate auth credentials",
                "path": settings.path.display().to_string(),
                "profile": context.profile,
                "before": redacted_auth_value(Some(&existing)),
                "after": redacted_auth_value(Some(&next)),
            },
            "status": local_auth_status(&context.profile, Some(&next)),
        }));
    }

    require_yes(cli, "rotating auth credentials")?;
    let profile = settings
        .config
        .profiles
        .get_mut(&context.profile)
        .expect("profile auth existence was checked");
    profile.auth = Some(next.clone());
    settings.save()?;

    let mut result = json!({
        "updated": true,
        "path": settings.path.display().to_string(),
        "profile": context.profile,
        "before": redacted_auth_value(Some(&existing)),
        "after": redacted_auth_value(Some(&next)),
        "status": local_auth_status(&context.profile, Some(&next)),
    });
    attach_auth_audit(client, cli, "auth.rotate", &next, &mut result).await;
    Ok(result)
}

fn validate_credentials(credentials: &AuthCredentialsArgs) -> Result<(), CliError> {
    if credentials.key_id.is_some() && credentials.key_id_env.is_some() {
        return Err(CliError::InvalidArgument {
            message: "use only one of --key-id or --key-id-env".to_string(),
        });
    }
    if credentials.private_key_path.is_some() && credentials.private_key_env.is_some() {
        return Err(CliError::InvalidArgument {
            message: "use only one of --private-key-path or --private-key-env".to_string(),
        });
    }
    if let Some(provider) = credentials.provider.as_deref() {
        validate_non_empty("provider", provider)?;
    }
    if let Some(key_id) = credentials.key_id.as_deref() {
        validate_non_empty("key-id", key_id)?;
    }
    if let Some(env) = credentials.key_id_env.as_deref() {
        validate_env_name("key-id-env", env)?;
    }
    if let Some(env) = credentials.private_key_env.as_deref() {
        validate_env_name("private-key-env", env)?;
    }
    Ok(())
}

fn build_added_auth(credentials: &AuthCredentialsArgs) -> Result<AuthConfig, CliError> {
    if credentials.key_id.is_none() && credentials.key_id_env.is_none() {
        return Err(CliError::InvalidArgument {
            message: "auth add requires --key-id or --key-id-env".to_string(),
        });
    }
    if credentials.private_key_path.is_none() && credentials.private_key_env.is_none() {
        return Err(CliError::InvalidArgument {
            message: "auth add requires --private-key-path or --private-key-env".to_string(),
        });
    }

    let now = Utc::now().to_rfc3339();
    Ok(AuthConfig {
        provider: credentials
            .provider
            .clone()
            .unwrap_or_else(|| "kalshi".to_string()),
        key_id: credentials.key_id.clone(),
        key_id_env: credentials.key_id_env.clone(),
        private_key_path: credentials
            .private_key_path
            .as_ref()
            .map(|path| path.display().to_string()),
        private_key_env: credentials.private_key_env.clone(),
        created_at: Some(now.clone()),
        updated_at: Some(now),
    })
}

fn build_rotated_auth(
    existing: &AuthConfig,
    credentials: &AuthCredentialsArgs,
) -> Result<AuthConfig, CliError> {
    let mut next = existing.clone();
    if let Some(provider) = credentials.provider.clone() {
        next.provider = provider;
    }
    if credentials.key_id.is_some() || credentials.key_id_env.is_some() {
        next.key_id = credentials.key_id.clone();
        next.key_id_env = credentials.key_id_env.clone();
    }
    if credentials.private_key_path.is_some() || credentials.private_key_env.is_some() {
        next.private_key_path = credentials
            .private_key_path
            .as_ref()
            .map(|path| path.display().to_string());
        next.private_key_env = credentials.private_key_env.clone();
    }
    next.updated_at = Some(Utc::now().to_rfc3339());

    if next.key_id.is_none() && next.key_id_env.is_none() {
        return Err(CliError::InvalidArgument {
            message: "rotated auth would not have a key id source".to_string(),
        });
    }
    if next.private_key_path.is_none() && next.private_key_env.is_none() {
        return Err(CliError::InvalidArgument {
            message: "rotated auth would not have a private key source".to_string(),
        });
    }

    Ok(next)
}

fn credentials_have_any_change(credentials: &AuthCredentialsArgs) -> bool {
    credentials.provider.is_some()
        || credentials.key_id.is_some()
        || credentials.key_id_env.is_some()
        || credentials.private_key_path.is_some()
        || credentials.private_key_env.is_some()
}

fn upsert_profile<'a>(
    settings: &'a mut LoadedSettings,
    context: &OutputContext,
) -> &'a mut ProfileConfig {
    settings
        .config
        .profiles
        .entry(context.profile.clone())
        .or_insert_with(|| ProfileConfig {
            daemon_url: Some(if context.daemon_url.is_empty() {
                DEFAULT_DAEMON_URL.to_string()
            } else {
                context.daemon_url.clone()
            }),
            kalshi_config: context.kalshi_config.clone(),
            ..ProfileConfig::default()
        })
}

fn local_auth_status(profile: &str, auth: Option<&AuthConfig>) -> Value {
    let Some(auth) = auth else {
        return json!({
            "profile": profile,
            "configured": false,
            "available": false,
            "auth": Value::Null,
            "sources": Value::Null,
            "warnings": ["no auth credentials configured for this profile"],
        });
    };

    let key_id_source = key_id_source_status(auth);
    let private_key_source = private_key_source_status(auth);
    let key_id_available = key_id_source
        .get("available")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let private_key_available = private_key_source
        .get("available")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let configured = auth.key_id.is_some() || auth.key_id_env.is_some();
    let private_configured = auth.private_key_path.is_some() || auth.private_key_env.is_some();
    let mut warnings = Vec::new();

    if auth.key_id.is_some() {
        warnings.push("key id is stored in omu.toml; prefer --key-id-env for shared environments");
    }
    if !key_id_available {
        warnings.push("key id source is not available");
    }
    if !private_key_available {
        warnings.push("private key source is not available");
    }

    json!({
        "profile": profile,
        "configured": configured && private_configured,
        "available": key_id_available && private_key_available,
        "auth": redacted_auth_value(Some(auth)),
        "sources": {
            "key_id": key_id_source,
            "private_key": private_key_source,
        },
        "warnings": warnings,
    })
}

fn key_id_source_status(auth: &AuthConfig) -> Value {
    if let Some(key_id) = auth.key_id.as_deref() {
        return json!({
            "kind": "inline",
            "configured": true,
            "available": !key_id.trim().is_empty(),
            "redacted": crate::settings::redact_identifier(key_id),
        });
    }
    if let Some(env) = auth.key_id_env.as_deref() {
        let set = std::env::var_os(env).is_some();
        return json!({
            "kind": "env",
            "configured": true,
            "available": set,
            "name": env,
        });
    }
    json!({
        "kind": Value::Null,
        "configured": false,
        "available": false,
    })
}

fn private_key_source_status(auth: &AuthConfig) -> Value {
    if let Some(path) = auth.private_key_path.as_deref() {
        let exists = Path::new(path).is_file();
        return json!({
            "kind": "path",
            "configured": true,
            "available": exists,
            "path": path,
        });
    }
    if let Some(env) = auth.private_key_env.as_deref() {
        let set = std::env::var_os(env).is_some();
        return json!({
            "kind": "env",
            "configured": true,
            "available": set,
            "name": env,
        });
    }
    json!({
        "kind": Value::Null,
        "configured": false,
        "available": false,
    })
}

async fn attach_auth_audit(
    client: &DaemonClient,
    cli: &Cli,
    command: &str,
    auth: &AuthConfig,
    result: &mut Value,
) {
    let audit = audit_event(
        client,
        cli,
        command,
        json!({ "auth": redacted_auth_value(Some(auth)) }),
        result.clone(),
    )
    .await;
    if let Value::Object(map) = result {
        map.insert("audit".to_string(), audit);
    }
}

fn validate_non_empty(name: &str, value: &str) -> Result<(), CliError> {
    if value.trim().is_empty() {
        Err(CliError::InvalidArgument {
            message: format!("{name} cannot be empty"),
        })
    } else {
        Ok(())
    }
}

fn validate_env_name(name: &str, value: &str) -> Result<(), CliError> {
    validate_non_empty(name, value)?;
    if value.contains('=') {
        return Err(CliError::InvalidArgument {
            message: format!("{name} should be an environment variable name, not an assignment"),
        });
    }
    Ok(())
}
