use super::daemon;
use crate::cli::{Cli, ConfigCommand, ConfigSubcommand};
use crate::client::DaemonClient;
use crate::error::CliError;
use crate::output::OutputContext;
use crate::settings::LoadedSettings;
use serde_json::{json, Value};

pub(super) async fn handle(
    cli: &Cli,
    context: &OutputContext,
    client: &DaemonClient,
    command: &ConfigCommand,
) -> Result<Value, CliError> {
    match command.command {
        ConfigSubcommand::Path => path(cli, context),
        ConfigSubcommand::Show => show(cli, context),
        ConfigSubcommand::Doctor => doctor(client, context).await,
    }
}

fn path(cli: &Cli, context: &OutputContext) -> Result<Value, CliError> {
    let settings = LoadedSettings::load(cli.config_dir.as_deref())?;
    Ok(json!({
        "path": settings.path.display().to_string(),
        "exists": settings.exists,
        "config_dir": cli.config_dir.as_ref().map(|p| p.display().to_string()),
        "profile": context.profile,
        "profile_exists": context.profile_exists,
        "daemon_url": context.daemon_url,
        "kalshi_config": context.kalshi_config,
    }))
}

fn show(cli: &Cli, context: &OutputContext) -> Result<Value, CliError> {
    let settings = LoadedSettings::load(cli.config_dir.as_deref())?;
    let resolved = settings.resolve(Some(&context.profile));
    Ok(json!({
        "path": settings.path.display().to_string(),
        "exists": settings.exists,
        "active_profile": resolved,
        "config": settings.config,
    }))
}

async fn doctor(client: &DaemonClient, context: &OutputContext) -> Result<Value, CliError> {
    match daemon::probe(client).await {
        Ok(status) => Ok(json!({
            "profile": context.profile,
            "profile_exists": context.profile_exists,
            "config_path": context.config_path,
            "kalshi_config": context.kalshi_config,
            "daemon_url": context.daemon_url,
            "daemon_reachable": true,
            "status": status,
        })),
        Err(err) => Ok(json!({
            "profile": context.profile,
            "profile_exists": context.profile_exists,
            "config_path": context.config_path,
            "kalshi_config": context.kalshi_config,
            "daemon_url": context.daemon_url,
            "daemon_reachable": false,
            "error": {
                "code": err.code(),
                "message": err.to_string(),
                "hint": err.hint(),
            }
        })),
    }
}
