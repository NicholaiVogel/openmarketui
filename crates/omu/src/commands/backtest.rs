use super::{dry_run_response, require_yes, to_value, with_audit};
use crate::cli::{BacktestCommand, BacktestCompareArgs, BacktestSubcommand, Cli};
use crate::client::DaemonClient;
use crate::error::CliError;
use crate::types::{BacktestRequest, BacktestStatusResponse};
use serde_json::{Map, Value, json};
use std::time::Duration;

pub(super) async fn handle(
    cli: &Cli,
    client: &DaemonClient,
    command: &BacktestCommand,
) -> Result<Value, CliError> {
    match &command.command {
        BacktestSubcommand::Run(args) => {
            let req = BacktestRequest {
                start: args.start.clone(),
                end: args.end.clone(),
                capital: args.capital,
                max_positions: args.max_positions,
                max_position: args.max_position,
                interval_hours: args.interval_hours,
                kelly_fraction: args.kelly_fraction,
                max_position_pct: args.max_position_pct,
                take_profit: args.take_profit,
                stop_loss: args.stop_loss,
                max_hold_hours: args.max_hold_hours,
                data_source: args.data_source.clone(),
            };
            if cli.dry_run {
                return dry_run_response("start backtest", "POST", "/api/backtest/run", &req);
            }
            client.post_json_empty("/api/backtest/run", &req).await?;
            let audit = super::audit_event(
                client,
                cli,
                "backtest.run",
                &req,
                json!({ "started": true, "attach": args.attach }),
            )
            .await;
            if args.attach {
                let mut result = wait_for_backtest(client).await?;
                if let Value::Object(ref mut map) = result {
                    map.insert("audit".to_string(), audit);
                    Ok(result)
                } else {
                    Ok(json!({ "result": result, "audit": audit }))
                }
            } else {
                Ok(json!({ "started": true, "audit": audit }))
            }
        }
        BacktestSubcommand::Status => client.get_value("/api/backtest/status").await,
        BacktestSubcommand::Summary => client.get_value("/api/backtest/result").await,
        BacktestSubcommand::List { limit } => {
            client
                .get_value(&format!("/api/backtest/runs?limit={limit}"))
                .await
        }
        BacktestSubcommand::Show { id } => get_backtest_run(client, id).await,
        BacktestSubcommand::Compare(args) => compare_backtest_runs(client, args).await,
        BacktestSubcommand::Stop => {
            if cli.dry_run {
                return dry_run_response(
                    "stop active backtest",
                    "POST",
                    "/api/backtest/stop",
                    serde_json::Value::Null,
                );
            }
            require_yes(cli, "stopping the active backtest")?;
            client.post_empty("/api/backtest/stop").await?;
            with_audit(
                client,
                cli,
                "backtest.stop",
                serde_json::Value::Null,
                json!({ "stopped": true }),
            )
            .await
        }
    }
}

async fn wait_for_backtest(client: &DaemonClient) -> Result<Value, CliError> {
    loop {
        let status: BacktestStatusResponse = client.get("/api/backtest/status").await?;
        match status.status.as_str() {
            "complete" => {
                let result = client.get_value("/api/backtest/result").await?;
                return Ok(json!({ "status": status, "result": result }));
            }
            "failed" => return to_value(status),
            _ => tokio::time::sleep(Duration::from_secs(2)).await,
        }
    }
}

async fn get_backtest_run(client: &DaemonClient, id: &str) -> Result<Value, CliError> {
    match client.get_value(&format!("/api/backtest/runs/{id}")).await {
        Ok(value) => Ok(value),
        Err(CliError::DaemonStatus { status, .. }) if status == reqwest::StatusCode::NOT_FOUND => {
            Err(CliError::NotFound {
                resource: "backtest run".to_string(),
                id: id.to_string(),
            })
        }
        Err(err) => Err(err),
    }
}

async fn compare_backtest_runs(
    client: &DaemonClient,
    args: &BacktestCompareArgs,
) -> Result<Value, CliError> {
    let baseline = get_backtest_run(client, &args.baseline).await?;
    let challenger = get_backtest_run(client, &args.challenger).await?;

    let mut delta = Map::new();
    for metric in [
        "total_return",
        "total_return_pct",
        "sharpe_ratio",
        "max_drawdown_pct",
        "win_rate",
    ] {
        if let (Some(base), Some(next)) = (
            metric_f64(&baseline, metric),
            metric_f64(&challenger, metric),
        ) {
            delta.insert(
                metric.to_string(),
                json!({
                    "baseline": base,
                    "challenger": next,
                    "delta": next - base,
                }),
            );
        }
    }

    if let (Some(base), Some(next)) = (
        metric_i64(&baseline, "total_trades"),
        metric_i64(&challenger, "total_trades"),
    ) {
        delta.insert(
            "total_trades".to_string(),
            json!({
                "baseline": base,
                "challenger": next,
                "delta": next - base,
            }),
        );
    }

    Ok(json!({
        "baseline": baseline,
        "challenger": challenger,
        "delta": delta,
    }))
}

fn metric_f64(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64).or_else(|| {
        value
            .get("result")
            .and_then(|result| result.get(key))
            .and_then(Value::as_f64)
    })
}

fn metric_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64).or_else(|| {
        value
            .get("result")
            .and_then(|result| result.get(key))
            .and_then(Value::as_i64)
    })
}
