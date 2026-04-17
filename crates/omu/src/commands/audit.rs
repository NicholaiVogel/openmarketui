use crate::cli::{AuditCommand, AuditSubcommand};
use crate::client::DaemonClient;
use crate::error::CliError;
use serde_json::Value;

pub(super) async fn handle(
    client: &DaemonClient,
    command: &AuditCommand,
) -> Result<Value, CliError> {
    match command.command {
        AuditSubcommand::List { limit } => {
            client.get_value(&format!("/api/audit?limit={limit}")).await
        }
    }
}
