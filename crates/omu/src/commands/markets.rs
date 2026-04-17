use super::{find_by_ticker, to_value};
use crate::cli::{MarketsCommand, MarketsSubcommand};
use crate::client::DaemonClient;
use crate::error::CliError;
use crate::types::MarketResponse;
use serde_json::Value;

pub(super) async fn handle(
    client: &DaemonClient,
    command: &MarketsCommand,
) -> Result<Value, CliError> {
    match &command.command {
        MarketsSubcommand::List { limit } => {
            client
                .get_value(&format!("/api/markets?limit={limit}"))
                .await
        }
        MarketsSubcommand::Show { ticker, limit } => find_by_ticker(
            client
                .get::<Vec<MarketResponse>>(&format!("/api/markets?limit={limit}"))
                .await?,
            ticker,
            "market",
        ),
        MarketsSubcommand::Search { query, limit } => {
            let query_lower = query.to_lowercase();
            let markets = client
                .get::<Vec<MarketResponse>>(&format!("/api/markets?limit={limit}"))
                .await?;
            let matches: Vec<_> = markets
                .into_iter()
                .filter(|m| {
                    m.ticker.to_lowercase().contains(&query_lower)
                        || m.title.to_lowercase().contains(&query_lower)
                        || m.category
                            .as_deref()
                            .unwrap_or_default()
                            .to_lowercase()
                            .contains(&query_lower)
                })
                .collect();
            to_value(matches)
        }
    }
}
