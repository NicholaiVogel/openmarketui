use super::{dry_run_response, with_audit};
use crate::cli::{Cli, IngestCommand, IngestSubcommand};
use crate::client::DaemonClient;
use crate::error::CliError;
use crate::types::DataFetchRequest;
use serde_json::{json, Value};

pub(super) async fn handle(
    cli: &Cli,
    client: &DaemonClient,
    command: &IngestCommand,
) -> Result<Value, CliError> {
    match &command.command {
        IngestSubcommand::Status => {
            let status = client.get_value("/api/data/status").await?;
            let available = client.get_value("/api/data/available").await?;
            Ok(json!({ "status": status, "available": available }))
        }
        IngestSubcommand::Fetch(args) => {
            let req = DataFetchRequest {
                start_date: args.start.clone(),
                end_date: args.end.clone(),
                trades_per_day: args.trades_per_day,
                fetch_markets: args.fetch_markets,
                fetch_trades: args.fetch_trades,
            };
            if cli.dry_run {
                return dry_run_response(
                    "start historical data fetch",
                    "POST",
                    "/api/data/fetch",
                    &req,
                );
            }
            let response = client
                .post_json::<Value, _>("/api/data/fetch", &req)
                .await?;
            with_audit(client, cli, "ingest.fetch", &req, response).await
        }
        IngestSubcommand::Cancel => {
            if cli.dry_run {
                return dry_run_response(
                    "cancel historical data fetch",
                    "POST",
                    "/api/data/cancel",
                    serde_json::Value::Null,
                );
            }
            client.post_empty("/api/data/cancel").await?;
            with_audit(
                client,
                cli,
                "ingest.cancel",
                serde_json::Value::Null,
                json!({ "cancel_requested": true }),
            )
            .await
        }
    }
}
