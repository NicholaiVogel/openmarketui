use super::{dry_run_response, find_by_ticker, require_yes, with_audit};
use crate::cli::{Cli, PositionResultArg, PositionsCommand, PositionsSubcommand};
use crate::client::DaemonClient;
use crate::error::CliError;
use crate::types::PositionResponse;
use serde_json::{json, Value};

pub(super) async fn handle(
    cli: &Cli,
    client: &DaemonClient,
    command: &PositionsCommand,
) -> Result<Value, CliError> {
    match &command.command {
        PositionsSubcommand::List => client.get_value("/api/positions").await,
        PositionsSubcommand::Show { ticker } => find_by_ticker(
            client
                .get::<Vec<PositionResponse>>("/api/positions")
                .await?,
            ticker,
            "position",
        ),
        PositionsSubcommand::Close { ticker } => close(cli, client, ticker).await,
        PositionsSubcommand::Redeem { ticker, result } => {
            redeem(cli, client, ticker.as_deref(), *result).await
        }
    }
}

async fn close(cli: &Cli, client: &DaemonClient, ticker: &str) -> Result<Value, CliError> {
    let path = format!("/api/positions/{ticker}/close");
    let request = json!({ "ticker": ticker });

    if cli.dry_run {
        return dry_run_response("close position", "POST", &path, &request);
    }

    require_yes(cli, "closing a position")?;

    match client.post_json::<Value, _>(&path, &request).await {
        Ok(response) => with_audit(client, cli, "positions.close", request, response).await,
        Err(CliError::DaemonStatus { status, .. }) if status == reqwest::StatusCode::NOT_FOUND => {
            Err(CliError::NotFound {
                resource: "position".to_string(),
                id: ticker.to_string(),
            })
        }
        Err(err) => Err(err),
    }
}

async fn redeem(
    cli: &Cli,
    client: &DaemonClient,
    ticker: Option<&str>,
    result: Option<PositionResultArg>,
) -> Result<Value, CliError> {
    if ticker.is_none() && result.is_some() {
        return Err(CliError::InvalidArgument {
            message: "--result requires a ticker; bulk redeem only redeems daemon-known resolved positions"
                .to_string(),
        });
    }

    let request = match result {
        Some(result) => json!({ "result": result.as_daemon_result() }),
        None => json!({}),
    };
    let path = ticker
        .map(|ticker| format!("/api/positions/{ticker}/redeem"))
        .unwrap_or_else(|| "/api/positions/redeem".to_string());
    let action = if ticker.is_some() {
        "redeem position"
    } else {
        "redeem resolved positions"
    };

    if cli.dry_run {
        return dry_run_response(action, "POST", &path, &request);
    }

    require_yes(cli, action)?;

    match client.post_json::<Value, _>(&path, &request).await {
        Ok(response) => with_audit(client, cli, "positions.redeem", request, response).await,
        Err(CliError::DaemonStatus { status, .. }) if status == reqwest::StatusCode::NOT_FOUND => {
            Err(CliError::NotFound {
                resource: "position".to_string(),
                id: ticker.unwrap_or("resolved").to_string(),
            })
        }
        Err(err) => Err(err),
    }
}
