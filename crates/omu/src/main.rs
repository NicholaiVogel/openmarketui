mod cli;
mod client;
mod commands;
mod error;
mod output;
mod settings;
mod types;

use clap::Parser;
use cli::{Cli, Commands, ProfilesSubcommand};
use commands::execute;
use output::{print_error, print_ok, OutputContext};
use settings::LoadedSettings;

#[tokio::main]
async fn main() {
    let mut cli = Cli::parse();
    let format = cli.format;
    let command_scoped_daemon_url = match &cli.command {
        Commands::Profiles(command) => match &command.command {
            ProfilesSubcommand::Create(args) => args.daemon_url.clone(),
            _ => None,
        },
        _ => None,
    };
    if command_scoped_daemon_url.is_some() && cli.daemon_url == command_scoped_daemon_url {
        cli.daemon_url = None;
    }

    let settings = match LoadedSettings::load(cli.config_dir.as_deref()) {
        Ok(settings) => settings,
        Err(err) => {
            let context = OutputContext {
                profile: cli.profile_name().to_string(),
                profile_exists: false,
                daemon_url: cli.daemon_url(),
                config_path: cli
                    .config_dir
                    .as_ref()
                    .map(|path| path.join("omu.toml").display().to_string())
                    .unwrap_or_else(|| "omu.toml".to_string()),
                kalshi_config: None,
                dry_run: cli.dry_run,
                trace_id: None,
            };
            let _ = print_error(format, &context, &err);
            std::process::exit(err.exit_code());
        }
    };

    let resolved = settings.resolve(cli.profile.as_deref());
    cli.profile = Some(resolved.name.clone());
    if cli.daemon_url.is_none() {
        cli.daemon_url = std::env::var("OPENMARKETUI_DAEMON_URL")
            .ok()
            .or_else(|| Some(resolved.daemon_url.clone()));
    }
    let policy_dry_run_applies =
        !matches!(cli.command, Commands::Config(_) | Commands::Profiles(_));
    if policy_dry_run_applies && resolved.dry_run_default {
        cli.dry_run = true;
    }
    cli.policy_require_yes = resolved.policy.require_yes;
    cli.policy_allow_live = resolved.policy.allow_live;
    cli.policy_max_position_usd = resolved.policy.max_position_usd;
    cli.policy_max_bankroll_usd = resolved.policy.max_bankroll_usd;

    let context = OutputContext {
        profile: resolved.name.clone(),
        profile_exists: resolved.exists,
        daemon_url: cli.daemon_url(),
        config_path: settings.path.display().to_string(),
        kalshi_config: resolved.kalshi_config.clone(),
        dry_run: cli.dry_run,
        trace_id: None,
    };

    match execute(&cli, &context).await {
        Ok(data) => {
            if let Err(err) = print_ok(format, &context, data) {
                eprintln!("failed to print output: {err}");
                std::process::exit(1);
            }
        }
        Err(err) => {
            let _ = print_error(format, &context, &err);
            std::process::exit(err.exit_code());
        }
    }
}
