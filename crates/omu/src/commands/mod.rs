mod audit;
mod auth;
mod backtest;
mod config;
mod daemon;
mod ingest;
mod markets;
mod pipeline;
mod portfolio;
mod positions;
mod profiles;
mod sessions;
mod trades;

use crate::cli::{Cli, Commands};
use crate::client::DaemonClient;
use crate::error::CliError;
use crate::output::OutputContext;
use crate::types::HasTicker;
use serde::Serialize;
use serde_json::{json, Value};

pub(crate) async fn execute(cli: &Cli, context: &OutputContext) -> Result<Value, CliError> {
    let client = DaemonClient::new(context.daemon_url.clone(), context.trace_id.clone())?;

    match &cli.command {
        Commands::Daemon(cmd) => daemon::handle(cli, context, &client, cmd).await,
        Commands::Overview => client.get_value("/api/snapshot").await,
        Commands::Portfolio(cmd) => portfolio::handle(&client, cmd).await,
        Commands::Positions(cmd) => positions::handle(cli, &client, cmd).await,
        Commands::Trades(cmd) => trades::handle(&client, cmd).await,
        Commands::Markets(cmd) => markets::handle(&client, cmd).await,
        Commands::Pipeline(cmd) => pipeline::handle(cli, &client, cmd).await,
        Commands::Ingest(cmd) => ingest::handle(cli, &client, cmd).await,
        Commands::Backtest(cmd) => backtest::handle(cli, &client, cmd).await,
        Commands::Sessions(cmd) => sessions::handle(cli, &client, cmd).await,
        Commands::Audit(cmd) => audit::handle(&client, cmd).await,
        Commands::Auth(cmd) => auth::handle(cli, context, &client, cmd).await,
        Commands::Config(cmd) => config::handle(cli, context, &client, cmd).await,
        Commands::Profiles(cmd) => profiles::handle(cli, context, cmd).await,
    }
}

pub(super) fn require_yes(cli: &Cli, action: &str) -> Result<(), CliError> {
    if cli.yes || !cli.policy_require_yes {
        Ok(())
    } else {
        Err(CliError::ConfirmationRequired {
            action: action.to_string(),
        })
    }
}

pub(super) fn dry_run_response<T>(
    action: &str,
    method: &str,
    path: &str,
    payload: T,
) -> Result<Value, CliError>
where
    T: Serialize,
{
    Ok(json!({
        "dry_run": true,
        "would": {
            "action": action,
            "method": method,
            "path": path,
            "payload": payload,
        }
    }))
}

pub(super) async fn audit_event<Rq, Rs>(
    client: &DaemonClient,
    cli: &Cli,
    command: &str,
    request: Rq,
    result: Rs,
) -> Value
where
    Rq: Serialize,
    Rs: Serialize,
{
    let event = json!({
        "actor": "cli",
        "command": command,
        "profile": cli.profile_name(),
        "dry_run": cli.dry_run,
        "request": serde_json::to_value(request).unwrap_or(Value::Null),
        "result": serde_json::to_value(result).unwrap_or(Value::Null),
        "trace_id": cli.trace_id.as_deref(),
    });

    match client.post_json::<Value, _>("/api/audit", &event).await {
        Ok(response) => json!({
            "recorded": true,
            "event": response,
        }),
        Err(err) => json!({
            "recorded": false,
            "error": {
                "code": err.code(),
                "message": err.to_string(),
                "hint": err.hint(),
            }
        }),
    }
}

pub(super) async fn with_audit<Rq, Rs>(
    client: &DaemonClient,
    cli: &Cli,
    command: &str,
    request: Rq,
    result: Rs,
) -> Result<Value, CliError>
where
    Rq: Serialize,
    Rs: Serialize,
{
    let request = serde_json::to_value(request)?;
    let mut result = serde_json::to_value(result)?;
    let audit = audit_event(client, cli, command, request, result.clone()).await;

    if let Value::Object(ref mut map) = result {
        map.insert("audit".to_string(), audit);
        Ok(result)
    } else {
        Ok(json!({
            "result": result,
            "audit": audit,
        }))
    }
}

pub(super) fn find_by_ticker<T>(
    items: Vec<T>,
    ticker: &str,
    resource: &str,
) -> Result<Value, CliError>
where
    T: Serialize + HasTicker,
{
    items
        .into_iter()
        .find(|item| item.ticker() == ticker)
        .map(to_value)
        .transpose()?
        .ok_or_else(|| CliError::NotFound {
            resource: resource.to_string(),
            id: ticker.to_string(),
        })
}

pub(super) fn to_value<T: Serialize>(value: T) -> Result<Value, CliError> {
    Ok(serde_json::to_value(value)?)
}
