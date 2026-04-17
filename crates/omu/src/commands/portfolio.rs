use crate::cli::{PortfolioCommand, PortfolioSubcommand};
use crate::client::DaemonClient;
use crate::error::CliError;
use serde_json::Value;

pub(super) async fn handle(
    client: &DaemonClient,
    command: &PortfolioCommand,
) -> Result<Value, CliError> {
    match command.command {
        PortfolioSubcommand::Summary => client.get_value("/api/portfolio").await,
        PortfolioSubcommand::History | PortfolioSubcommand::EquityCurve => {
            client.get_value("/api/equity").await
        }
    }
}
