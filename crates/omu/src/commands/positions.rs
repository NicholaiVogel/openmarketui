use super::{dry_run_response, find_by_ticker, require_yes, with_audit};
use crate::cli::{Cli, PositionsCommand, PositionsSubcommand};
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
