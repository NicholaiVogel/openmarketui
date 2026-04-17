use super::require_yes;
use crate::cli::{Cli, ProfileCreateArgs, ProfilePolicyArgs, ProfilesCommand, ProfilesSubcommand};
use crate::error::CliError;
use crate::output::OutputContext;
use crate::settings::{
    redacted_auth_value, LoadedSettings, PolicyConfig, ProfileConfig, DEFAULT_DAEMON_URL,
    DEFAULT_PROFILE,
};
use serde_json::{json, Value};

pub(super) async fn handle(
    cli: &Cli,
    context: &OutputContext,
    command: &ProfilesCommand,
) -> Result<Value, CliError> {
    match &command.command {
        ProfilesSubcommand::List => list(cli, context),
        ProfilesSubcommand::Show { name } => show(cli, context, name.as_deref()),
        ProfilesSubcommand::Create(args) => create(cli, context, args),
        ProfilesSubcommand::SetDefault { name } => set_default(cli, context, name),
        ProfilesSubcommand::Policy(args) => policy(cli, context, args),
    }
}

fn list(cli: &Cli, context: &OutputContext) -> Result<Value, CliError> {
    let settings = LoadedSettings::load(cli.config_dir.as_deref())?;
    let profiles: Vec<Value> = settings
        .config
        .profiles
        .iter()
        .map(|(name, profile)| {
            json!({
                "name": name,
                "active": name == &context.profile,
                "default": name == &settings.config.default_profile,
                "daemon_url": profile.daemon_url.as_deref().unwrap_or(DEFAULT_DAEMON_URL),
                "kalshi_config": profile.kalshi_config,
                "policy": profile.policy,
                "auth": redacted_auth_value(profile.auth.as_ref()),
            })
        })
        .collect();

    Ok(json!({
        "path": settings.path.display().to_string(),
        "exists": settings.exists,
        "default_profile": settings.config.default_profile,
        "active_profile": context.profile,
        "active_profile_exists": context.profile_exists,
        "implicit_default_profile": !settings.config.profiles.contains_key(DEFAULT_PROFILE),
        "profiles": profiles,
    }))
}

fn show(cli: &Cli, context: &OutputContext, name: Option<&str>) -> Result<Value, CliError> {
    let settings = LoadedSettings::load(cli.config_dir.as_deref())?;
    let name = name.unwrap_or(&context.profile);

    if let Some(profile) = settings.config.profiles.get(name) {
        return Ok(json!({
            "name": name,
            "active": name == context.profile,
            "default": name == settings.config.default_profile,
            "exists": true,
            "daemon_url": profile.daemon_url.as_deref().unwrap_or(DEFAULT_DAEMON_URL),
            "kalshi_config": profile.kalshi_config,
            "policy": profile.policy,
            "auth": redacted_auth_value(profile.auth.as_ref()),
        }));
    }

    if name == DEFAULT_PROFILE && !settings.exists {
        let resolved = settings.resolve(Some(name));
        return Ok(json!({
            "name": name,
            "active": name == context.profile,
            "default": true,
            "exists": false,
            "implicit": true,
            "daemon_url": resolved.daemon_url,
            "kalshi_config": resolved.kalshi_config,
            "policy": resolved.policy,
            "auth": null,
        }));
    }

    Err(CliError::NotFound {
        resource: "profile".to_string(),
        id: name.to_string(),
    })
}

fn create(cli: &Cli, context: &OutputContext, args: &ProfileCreateArgs) -> Result<Value, CliError> {
    validate_profile_name(&args.name)?;
    let mut settings = LoadedSettings::load(cli.config_dir.as_deref())?;
    let already_exists = settings.config.profiles.contains_key(&args.name);
    let profile = ProfileConfig {
        daemon_url: args.daemon_url.clone(),
        kalshi_config: args
            .kalshi_config
            .as_ref()
            .map(|path| path.display().to_string()),
        policy: PolicyConfig {
            dry_run_default: args.dry_run_default,
            allow_live: args.allow_live,
            ..PolicyConfig::default()
        },
        auth: None,
    };

    if cli.dry_run {
        return Ok(json!({
            "dry_run": true,
            "would": {
                "action": if already_exists { "overwrite profile" } else { "create profile" },
                "path": settings.path.display().to_string(),
                "profile": args.name,
                "config": profile,
            },
            "active_profile": context.profile,
        }));
    }

    require_yes(
        cli,
        if already_exists {
            "overwriting a profile"
        } else {
            "creating a profile"
        },
    )?;

    settings.config.profiles.insert(args.name.clone(), profile);
    if !settings.exists && settings.config.default_profile == DEFAULT_PROFILE {
        settings.config.default_profile = args.name.clone();
    }
    settings.save()?;

    Ok(json!({
        "created": !already_exists,
        "overwritten": already_exists,
        "path": settings.path.display().to_string(),
        "profile": args.name,
        "default_profile": settings.config.default_profile,
    }))
}

fn set_default(cli: &Cli, context: &OutputContext, name: &str) -> Result<Value, CliError> {
    validate_profile_name(name)?;
    let mut settings = LoadedSettings::load(cli.config_dir.as_deref())?;
    ensure_profile_exists(&settings, name)?;

    if cli.dry_run {
        return Ok(json!({
            "dry_run": true,
            "would": {
                "action": "set default profile",
                "path": settings.path.display().to_string(),
                "from": settings.config.default_profile,
                "to": name,
            },
            "active_profile": context.profile,
        }));
    }

    require_yes(cli, "setting the default profile")?;
    let previous = settings.config.default_profile.clone();
    settings.config.default_profile = name.to_string();
    settings.save()?;

    Ok(json!({
        "updated": previous != name,
        "path": settings.path.display().to_string(),
        "previous": previous,
        "default_profile": settings.config.default_profile,
    }))
}

fn policy(cli: &Cli, context: &OutputContext, args: &ProfilePolicyArgs) -> Result<Value, CliError> {
    validate_policy_args(args)?;
    let mut settings = LoadedSettings::load(cli.config_dir.as_deref())?;
    let name = args.name.as_deref().unwrap_or(&context.profile).to_string();
    validate_profile_name(&name)?;
    ensure_profile_exists(&settings, &name)?;

    let before = settings
        .config
        .profiles
        .get(&name)
        .map(|profile| profile.policy.clone())
        .expect("profile existence was checked");

    if !policy_has_changes(args) {
        return Ok(json!({
            "path": settings.path.display().to_string(),
            "profile": name,
            "policy": before,
            "updated": false,
        }));
    }

    let mut after = before.clone();
    apply_policy_args(&mut after, args)?;

    if cli.dry_run {
        return Ok(json!({
            "dry_run": true,
            "would": {
                "action": "update profile policy",
                "path": settings.path.display().to_string(),
                "profile": name,
                "before": before,
                "after": after,
            },
            "active_profile": context.profile,
        }));
    }

    require_yes(cli, "updating profile policy")?;
    if let Some(profile) = settings.config.profiles.get_mut(&name) {
        profile.policy = after.clone();
    }
    settings.save()?;

    Ok(json!({
        "updated": true,
        "path": settings.path.display().to_string(),
        "profile": name,
        "before": before,
        "after": after,
    }))
}

fn ensure_profile_exists(settings: &LoadedSettings, name: &str) -> Result<(), CliError> {
    if settings.config.profiles.contains_key(name) {
        Ok(())
    } else {
        Err(CliError::NotFound {
            resource: "profile".to_string(),
            id: name.to_string(),
        })
    }
}

fn validate_profile_name(name: &str) -> Result<(), CliError> {
    if name.trim().is_empty() {
        return Err(CliError::InvalidArgument {
            message: "profile name cannot be empty".to_string(),
        });
    }
    if name.contains('/') || name.contains('\\') {
        return Err(CliError::InvalidArgument {
            message: "profile name cannot contain path separators".to_string(),
        });
    }
    Ok(())
}

fn validate_policy_args(args: &ProfilePolicyArgs) -> Result<(), CliError> {
    if args.allow_live && args.deny_live {
        return Err(CliError::InvalidArgument {
            message: "use only one of --allow-live or --deny-live".to_string(),
        });
    }
    if args.dry_run_default && args.no_dry_run_default {
        return Err(CliError::InvalidArgument {
            message: "use only one of --dry-run-default or --no-dry-run-default".to_string(),
        });
    }
    if args.require_yes && args.no_require_yes {
        return Err(CliError::InvalidArgument {
            message: "use only one of --require-yes or --no-require-yes".to_string(),
        });
    }
    if args.max_position_usd.is_some() && args.clear_max_position_usd {
        return Err(CliError::InvalidArgument {
            message: "use only one of --max-position-usd or --clear-max-position-usd".to_string(),
        });
    }
    if args.max_bankroll_usd.is_some() && args.clear_max_bankroll_usd {
        return Err(CliError::InvalidArgument {
            message: "use only one of --max-bankroll-usd or --clear-max-bankroll-usd".to_string(),
        });
    }
    if let Some(value) = args.max_position_usd {
        validate_non_negative_finite("max-position-usd", value)?;
    }
    if let Some(value) = args.max_bankroll_usd {
        validate_non_negative_finite("max-bankroll-usd", value)?;
    }
    Ok(())
}

fn validate_non_negative_finite(name: &str, value: f64) -> Result<(), CliError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(CliError::InvalidArgument {
            message: format!("{name} must be a finite non-negative number"),
        })
    }
}

fn policy_has_changes(args: &ProfilePolicyArgs) -> bool {
    args.allow_live
        || args.deny_live
        || args.dry_run_default
        || args.no_dry_run_default
        || args.max_position_usd.is_some()
        || args.max_bankroll_usd.is_some()
        || args.clear_max_position_usd
        || args.clear_max_bankroll_usd
        || args.require_yes
        || args.no_require_yes
}

fn apply_policy_args(policy: &mut PolicyConfig, args: &ProfilePolicyArgs) -> Result<(), CliError> {
    if args.allow_live {
        policy.allow_live = true;
    }
    if args.deny_live {
        policy.allow_live = false;
    }
    if args.dry_run_default {
        policy.dry_run_default = true;
    }
    if args.no_dry_run_default {
        policy.dry_run_default = false;
    }
    if let Some(value) = args.max_position_usd {
        validate_non_negative_finite("max-position-usd", value)?;
        policy.max_position_usd = Some(value);
    }
    if args.clear_max_position_usd {
        policy.max_position_usd = None;
    }
    if let Some(value) = args.max_bankroll_usd {
        validate_non_negative_finite("max-bankroll-usd", value)?;
        policy.max_bankroll_usd = Some(value);
    }
    if args.clear_max_bankroll_usd {
        policy.max_bankroll_usd = None;
    }
    if args.require_yes {
        policy.require_yes = true;
    }
    if args.no_require_yes {
        policy.require_yes = false;
    }
    Ok(())
}
