use super::{dry_run_response, require_yes, to_value, with_audit};
use crate::cli::{
    Cli, DecisionsSubcommand, FiltersSubcommand, PipelineCommand, PipelineSubcommand,
    ScorersSubcommand,
};
use crate::client::DaemonClient;
use crate::error::CliError;
use crate::types::{BedResponse, BedWithSpecimens, ScorerToggleRequest, SpecimenResponse};
use serde_json::{json, Value};

pub(super) async fn handle(
    cli: &Cli,
    client: &DaemonClient,
    command: &PipelineCommand,
) -> Result<Value, CliError> {
    match &command.command {
        PipelineSubcommand::Status => status(client).await,
        PipelineSubcommand::Scorers(scorers) => match &scorers.command {
            ScorersSubcommand::List => scorers_list(client).await,
            ScorersSubcommand::Show { name } => scorer_show(client, name).await,
            ScorersSubcommand::Enable { name } => scorer_toggle(cli, client, name, true).await,
            ScorersSubcommand::Disable { name } => scorer_toggle(cli, client, name, false).await,
        },
        PipelineSubcommand::Filters(filters) => match &filters.command {
            FiltersSubcommand::List => filters_list(client).await,
            FiltersSubcommand::Show { name } => filter_show(client, name).await,
        },
        PipelineSubcommand::Decisions(decisions) => match &decisions.command {
            DecisionsSubcommand::List { limit } => {
                client
                    .get_value(&format!("/api/decisions?limit={limit}"))
                    .await
            }
            DecisionsSubcommand::Show { id } => {
                client.get_value(&format!("/api/decisions/{id}")).await
            }
            DecisionsSubcommand::Ticker { ticker, limit } => {
                client
                    .get_value(&format!("/api/markets/{ticker}/decisions?limit={limit}"))
                    .await
            }
        },
    }
}

async fn status(client: &DaemonClient) -> Result<Value, CliError> {
    let status = client.get_value("/api/status").await?;
    let garden = client.get_value("/api/garden/status").await?;
    let filters = client
        .get_value("/api/filters")
        .await
        .unwrap_or(Value::Null);
    Ok(json!({ "engine": status, "garden": garden, "filters": filters }))
}

async fn scorers_list(client: &DaemonClient) -> Result<Value, CliError> {
    let beds = client.get::<Vec<BedResponse>>("/api/beds").await?;
    let mut output = Vec::with_capacity(beds.len());
    for bed in beds {
        let specimens = client
            .get::<Vec<SpecimenResponse>>(&format!("/api/beds/{}/specimens", bed.name))
            .await
            .unwrap_or_default();
        output.push(BedWithSpecimens { bed, specimens });
    }
    to_value(output)
}

async fn scorer_show(client: &DaemonClient, name: &str) -> Result<Value, CliError> {
    let value = scorers_list(client).await?;
    let beds: Vec<BedWithSpecimens> = serde_json::from_value(value)?;
    for bed in beds {
        for specimen in bed.specimens {
            if specimen.name == name {
                return to_value(specimen);
            }
        }
    }
    Err(CliError::NotFound {
        resource: "scorer".to_string(),
        id: name.to_string(),
    })
}

async fn filters_list(client: &DaemonClient) -> Result<Value, CliError> {
    client.get_value("/api/filters").await
}

async fn filter_show(client: &DaemonClient, name: &str) -> Result<Value, CliError> {
    match client.get_value(&format!("/api/filters/{name}")).await {
        Ok(value) => Ok(value),
        Err(CliError::DaemonStatus { status, .. }) if status == reqwest::StatusCode::NOT_FOUND => {
            Err(CliError::NotFound {
                resource: "filter".to_string(),
                id: name.to_string(),
            })
        }
        Err(err) => Err(err),
    }
}

async fn scorer_toggle(
    cli: &Cli,
    client: &DaemonClient,
    name: &str,
    enabled: bool,
) -> Result<Value, CliError> {
    let path = format!("/api/control/scorers/{name}");
    let payload = ScorerToggleRequest { enabled };

    if cli.dry_run {
        return dry_run_response(
            if enabled {
                "enable scorer"
            } else {
                "disable scorer"
            },
            "POST",
            &path,
            &payload,
        );
    }

    require_yes(
        cli,
        if enabled {
            "enabling a scorer"
        } else {
            "disabling a scorer"
        },
    )?;
    client.post_json_empty(&path, &payload).await?;
    with_audit(
        client,
        cli,
        if enabled {
            "pipeline.scorers.enable"
        } else {
            "pipeline.scorers.disable"
        },
        json!({ "name": name, "enabled": enabled }),
        json!({ "name": name, "enabled": enabled }),
    )
    .await
}
