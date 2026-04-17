use crate::cli::{TradesCommand, TradesSubcommand};
use crate::client::DaemonClient;
use crate::error::CliError;
use serde_json::Value;

pub(super) async fn handle(
    client: &DaemonClient,
    command: &TradesCommand,
) -> Result<Value, CliError> {
    match command.command {
        TradesSubcommand::List => client.get_value("/api/trades").await,
    }
}
