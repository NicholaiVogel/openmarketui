use super::{dry_run_response, require_yes, with_audit};
use crate::cli::{Cli, SessionCreateArgs, SessionModeArg, SessionsCommand, SessionsSubcommand};
use crate::client::DaemonClient;
use crate::error::CliError;
use crate::types::{SessionConfig, SessionFeeConfig, SessionStartRequest};
use serde_json::{json, Value};

pub(super) async fn handle(
    cli: &Cli,
    client: &DaemonClient,
    command: &SessionsCommand,
) -> Result<Value, CliError> {
    match &command.command {
        SessionsSubcommand::List => list(client).await,
        SessionsSubcommand::Show => client.get_value("/api/session/status").await,
        SessionsSubcommand::Create(args) => create(cli, client, args).await,
        SessionsSubcommand::Stop => stop(cli, client).await,
    }
}

async fn list(client: &DaemonClient) -> Result<Value, CliError> {
    let active = client.get_value("/api/session/status").await?;
    Ok(json!({
        "active": active,
        "history": [],
        "history_available": false,
    }))
}

async fn create(
    cli: &Cli,
    client: &DaemonClient,
    args: &SessionCreateArgs,
) -> Result<Value, CliError> {
    validate_session_args(args)?;
    let req = build_session_start_request(args);
    let policy = policy_report(cli, args);

    if cli.dry_run {
        return Ok(json!({
            "dry_run": true,
            "would": {
                "action": "create session",
                "method": "POST",
                "path": "/api/session/start",
                "payload": req,
            },
            "policy": policy,
        }));
    }

    enforce_session_policy(cli, args)?;
    require_yes(cli, "creating a session")?;

    client.post_json_empty("/api/session/start", &req).await?;
    let status = client.get_value("/api/session/status").await?;
    with_audit(client, cli, "sessions.create", &req, status).await
}

async fn stop(cli: &Cli, client: &DaemonClient) -> Result<Value, CliError> {
    if cli.dry_run {
        return dry_run_response(
            "stop active session",
            "POST",
            "/api/session/stop",
            serde_json::Value::Null,
        );
    }
    require_yes(cli, "stopping the active session")?;
    client.post_empty("/api/session/stop").await?;
    with_audit(
        client,
        cli,
        "sessions.stop",
        serde_json::Value::Null,
        json!({ "stopped": true }),
    )
    .await
}

fn build_session_start_request(args: &SessionCreateArgs) -> SessionStartRequest {
    SessionStartRequest {
        mode: args.mode.as_daemon_mode().to_string(),
        config: SessionConfig {
            initial_capital: args.initial_capital,
            max_positions: args.max_positions,
            kelly_fraction: args.kelly_fraction,
            max_position_pct: args.max_position_pct,
            take_profit_pct: args.take_profit_pct,
            stop_loss_pct: args.stop_loss_pct,
            max_hold_hours: args.max_hold_hours,
            min_time_to_close_hours: args.min_time_to_close_hours,
            max_time_to_close_hours: args.max_time_to_close_hours,
            cash_reserve_pct: args.cash_reserve_pct,
            max_entries_per_tick: args.max_entries_per_tick,
            fees: SessionFeeConfig {
                taker_rate: args.taker_rate,
                maker_rate: args.maker_rate,
                max_per_contract: args.max_fee_per_contract,
                assume_taker: args.assume_taker,
                min_edge_after_fees: args.min_edge_after_fees,
            },
            backtest_start: args.backtest_start.clone(),
            backtest_end: args.backtest_end.clone(),
            backtest_interval_hours: matches!(args.mode, SessionModeArg::Backtest)
                .then_some(args.backtest_interval_hours),
        },
    }
}

fn validate_session_args(args: &SessionCreateArgs) -> Result<(), CliError> {
    validate_positive("initial-capital", args.initial_capital)?;
    validate_positive("kelly-fraction", args.kelly_fraction)?;
    validate_positive("max-position-pct", args.max_position_pct)?;
    validate_positive("take-profit-pct", args.take_profit_pct)?;
    validate_positive("stop-loss-pct", args.stop_loss_pct)?;
    validate_non_negative("cash-reserve-pct", args.cash_reserve_pct)?;
    validate_non_negative("taker-rate", args.taker_rate)?;
    validate_non_negative("maker-rate", args.maker_rate)?;
    validate_non_negative("max-fee-per-contract", args.max_fee_per_contract)?;
    validate_non_negative("min-edge-after-fees", args.min_edge_after_fees)?;

    if args.max_positions == 0 {
        return Err(CliError::InvalidArgument {
            message: "max-positions must be greater than zero".to_string(),
        });
    }
    if args.max_entries_per_tick == 0 {
        return Err(CliError::InvalidArgument {
            message: "max-entries-per-tick must be greater than zero".to_string(),
        });
    }
    if args.max_hold_hours <= 0 {
        return Err(CliError::InvalidArgument {
            message: "max-hold-hours must be greater than zero".to_string(),
        });
    }
    if args.min_time_to_close_hours < 0 {
        return Err(CliError::InvalidArgument {
            message: "min-time-to-close-hours cannot be negative".to_string(),
        });
    }
    if args.max_time_to_close_hours < args.min_time_to_close_hours {
        return Err(CliError::InvalidArgument {
            message:
                "max-time-to-close-hours must be greater than or equal to min-time-to-close-hours"
                    .to_string(),
        });
    }
    if args.backtest_interval_hours <= 0 {
        return Err(CliError::InvalidArgument {
            message: "backtest-interval-hours must be greater than zero".to_string(),
        });
    }

    match args.mode {
        SessionModeArg::Backtest => {
            if args.backtest_start.is_none() || args.backtest_end.is_none() {
                return Err(CliError::InvalidArgument {
                    message: "backtest sessions require --backtest-start and --backtest-end"
                        .to_string(),
                });
            }
        }
        SessionModeArg::Paper | SessionModeArg::Live => {}
    }

    Ok(())
}

fn enforce_session_policy(cli: &Cli, args: &SessionCreateArgs) -> Result<(), CliError> {
    if matches!(args.mode, SessionModeArg::Live) {
        let reason = if cli.policy_allow_live {
            "live trading requires auth, circuit breaker, bankroll, audit, and trace gates that are not implemented yet"
        } else {
            "selected profile does not permit live trading"
        };
        return Err(CliError::PolicyBlocked {
            action: "live session creation".to_string(),
            reason: reason.to_string(),
        });
    }

    if let Some(max_bankroll) = cli.policy_max_bankroll_usd {
        if args.initial_capital > max_bankroll {
            return Err(CliError::PolicyBlocked {
                action: "session creation".to_string(),
                reason: format!(
                    "initial capital {} exceeds profile max_bankroll_usd {}",
                    args.initial_capital, max_bankroll
                ),
            });
        }
    }

    if let Some(max_position) = cli.policy_max_position_usd {
        let requested_max_position = args.initial_capital * args.max_position_pct;
        if requested_max_position > max_position {
            return Err(CliError::PolicyBlocked {
                action: "session creation".to_string(),
                reason: format!(
                    "configured max position {} exceeds profile max_position_usd {}",
                    requested_max_position, max_position
                ),
            });
        }
    }

    Ok(())
}

fn policy_report(cli: &Cli, args: &SessionCreateArgs) -> Value {
    let requested_max_position = args.initial_capital * args.max_position_pct;
    json!({
        "allow_live": cli.policy_allow_live,
        "max_position_usd": cli.policy_max_position_usd,
        "max_bankroll_usd": cli.policy_max_bankroll_usd,
        "requested_bankroll_usd": args.initial_capital,
        "requested_max_position_usd": requested_max_position,
        "live_blocked_until_gates_exist": matches!(args.mode, SessionModeArg::Live),
    })
}

fn validate_positive(name: &str, value: f64) -> Result<(), CliError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(CliError::InvalidArgument {
            message: format!("{name} must be a finite positive number"),
        })
    }
}

fn validate_non_negative(name: &str, value: f64) -> Result<(), CliError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(CliError::InvalidArgument {
            message: format!("{name} must be a finite non-negative number"),
        })
    }
}
