//! Kalshi trading CLI

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use clap::{Parser, Subcommand};
use pm_core::{BacktestConfig, Decision, DecisionAction, ExitConfig, Side};
use pm_engine::PositionSizingConfig;
use pm_kalshi::{
    api::KalshiClient,
    backtest::{Backtester, RandomBaseline},
    config::AppConfig,
    data::{ingest_csv_to_sqlite, DataFetcher, FetchState, HistoricalData},
    engine::PaperTradingEngine,
    metrics::BacktestResult,
    sources::PaperExecutor,
    web,
};
use rust_decimal::Decimal;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "kalshi")]
#[command(about = "trading engine for kalshi prediction markets")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(short, long, default_value = "data")]
        data_dir: PathBuf,

        /// Path to SQLite database with ingested data (skips CSV loading)
        #[arg(long)]
        db: Option<PathBuf>,

        #[arg(long)]
        start: String,

        #[arg(long)]
        end: String,

        #[arg(long, default_value = "10000")]
        capital: f64,

        #[arg(long, default_value = "100")]
        max_position: u64,

        #[arg(long, default_value = "100")]
        max_positions: usize,

        #[arg(long, default_value = "1")]
        interval_hours: i64,

        #[arg(long, default_value = "results")]
        output_dir: PathBuf,

        #[arg(long)]
        compare_random: bool,

        #[arg(long, default_value = "0.40")]
        kelly_fraction: f64,

        #[arg(long, default_value = "0.30")]
        max_position_pct: f64,

        #[arg(long, default_value = "0.50")]
        take_profit: f64,

        #[arg(long, default_value = "0.99")]
        stop_loss: f64,

        #[arg(long, default_value = "48")]
        max_hold_hours: i64,
    },

    /// Ingest CSV data into SQLite for faster backtesting
    Ingest {
        #[arg(short, long, default_value = "data")]
        data_dir: PathBuf,

        /// Path to SQLite database to write to
        #[arg(long, default_value = "data/historical.db")]
        db: PathBuf,
    },

    Paper {
        #[arg(short, long, default_value = "config.toml")]
        config: PathBuf,
    },

    Summary {
        #[arg(short, long)]
        results_file: PathBuf,
    },
}

pub fn parse_date(s: &str) -> Result<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()));
    }

    Err(anyhow::anyhow!("could not parse date: {}", s))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "kalshi=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            data_dir,
            db,
            start,
            end,
            capital,
            max_position,
            max_positions,
            interval_hours,
            output_dir,
            compare_random,
            kelly_fraction,
            max_position_pct,
            take_profit,
            stop_loss,
            max_hold_hours,
        } => {
            let start_time = parse_date(&start).context("parsing start date")?;
            let end_time = parse_date(&end).context("parsing end date")?;

            let data = if let Some(db_path) = &db {
                info!(
                    db = %db_path.display(),
                    start = %start_time,
                    end = %end_time,
                    capital = capital,
                    "loading data from sqlite"
                );

                let store =
                    pm_store::SqliteStore::new(db_path.to_str().context("invalid db path")?)
                        .await
                        .context("opening sqlite database")?;

                Arc::new(
                    HistoricalData::load_sqlite(&store, start_time, end_time)
                        .await
                        .context("loading historical data from sqlite")?,
                )
            } else {
                info!(
                    data_dir = %data_dir.display(),
                    start = %start_time,
                    end = %end_time,
                    capital = capital,
                    "loading data from CSV"
                );

                Arc::new(HistoricalData::load(&data_dir).context("loading historical data")?)
            };

            info!(
                markets = data.markets.len(),
                trades = data.trades.len(),
                "data loaded"
            );

            let config = BacktestConfig {
                start_time,
                end_time,
                interval: chrono::TimeDelta::hours(interval_hours),
                initial_capital: Decimal::try_from(capital).unwrap(),
                max_position_size: max_position,
                max_positions,
            };

            let sizing_config = PositionSizingConfig {
                kelly_fraction,
                max_position_pct,
                min_position_size: 10,
                max_position_size: max_position,
            };

            let exit_config = ExitConfig {
                take_profit_pct: take_profit,
                stop_loss_pct: stop_loss,
                max_hold_hours,
                score_reversal_threshold: -0.3,
            };

            let backtester =
                Backtester::with_configs(config.clone(), data.clone(), sizing_config, exit_config);
            let result = backtester.run().await;

            println!("{}", result.summary());

            std::fs::create_dir_all(&output_dir)?;
            let result_path = output_dir.join("backtest_result.json");
            let json = serde_json::to_string_pretty(&result)?;
            std::fs::write(&result_path, json)?;
            info!(path = %result_path.display(), "results saved");

            if compare_random {
                println!("\n--- random baseline ---\n");
                let baseline = RandomBaseline::new(config, data);
                let baseline_result = baseline.run().await;
                println!("{}", baseline_result.summary());

                let baseline_path = output_dir.join("baseline_result.json");
                let json = serde_json::to_string_pretty(&baseline_result)?;
                std::fs::write(&baseline_path, json)?;

                println!("\n--- comparison ---\n");
                println!(
                    "strategy return: {:.2}% vs baseline: {:.2}%",
                    result.total_return_pct, baseline_result.total_return_pct
                );
                println!(
                    "strategy sharpe: {:.3} vs baseline: {:.3}",
                    result.sharpe_ratio, baseline_result.sharpe_ratio
                );
                println!(
                    "strategy win rate: {:.1}% vs baseline: {:.1}%",
                    result.win_rate, baseline_result.win_rate
                );
            }

            Ok(())
        }

        Commands::Ingest { data_dir, db } => {
            info!(
                data_dir = %data_dir.display(),
                db = %db.display(),
                "ingesting CSV data into sqlite"
            );

            let store = pm_store::SqliteStore::new(db.to_str().context("invalid db path")?)
                .await
                .context("opening sqlite database")?;

            ingest_csv_to_sqlite(&data_dir, &store).await?;

            let markets = store.count_historical_markets().await?;
            let trades = store.count_historical_trades().await?;
            info!(markets, trades, db = %db.display(), "ingest complete");

            Ok(())
        }

        Commands::Paper {
            config: config_path,
        } => run_paper(config_path).await,

        Commands::Summary { results_file } => {
            let content = std::fs::read_to_string(&results_file).context("reading results file")?;
            let result: BacktestResult =
                serde_json::from_str(&content).context("parsing results")?;

            println!("{}", result.summary());

            Ok(())
        }
    }
}

async fn run_paper(config_path: PathBuf) -> Result<()> {
    let app_config = AppConfig::load(&config_path).context("loading config")?;

    info!(
        mode = ?app_config.mode,
        poll_secs = app_config.kalshi.poll_interval_secs,
        capital = app_config.trading.initial_capital,
        "starting paper trading"
    );

    let store = Arc::new(
        pm_store::SqliteStore::new(&app_config.persistence.db_path)
            .await
            .context("initializing SQLite store")?,
    );

    let client = Arc::new(KalshiClient::new(&app_config.kalshi));

    let sizing_config = PositionSizingConfig {
        kelly_fraction: app_config.trading.kelly_fraction,
        max_position_pct: app_config.trading.max_position_pct,
        min_position_size: 10,
        max_position_size: 1000,
    };

    let exit_config = ExitConfig {
        take_profit_pct: app_config.trading.take_profit_pct.unwrap_or(0.50),
        stop_loss_pct: app_config.trading.stop_loss_pct.unwrap_or(0.99),
        max_hold_hours: app_config.trading.max_hold_hours.unwrap_or(48),
        score_reversal_threshold: -0.3,
    };

    let fee_config: pm_engine::FeeConfig = app_config.fees.clone().into();

    let executor = Arc::new(PaperExecutor::new(
        1000,
        sizing_config,
        exit_config,
        fee_config,
        store.clone(),
    ));

    let engine = PaperTradingEngine::new(app_config.clone(), store.clone(), executor, client)
        .await
        .context("initializing engine")?;

    let shutdown_tx = engine.shutdown_handle();

    let engine = Arc::new(engine);

    let (updates_tx, _) = tokio::sync::broadcast::channel(256);

    let specimens = Arc::new(tokio::sync::RwLock::new(web::create_default_specimens()));

    if app_config.web.enabled {
        let data_dir = PathBuf::from("data");
        let historical_store = Arc::new(
            pm_store::SqliteStore::new(
                data_dir
                    .join("historical.db")
                    .to_str()
                    .unwrap_or("data/historical.db"),
            )
            .await
            .context("opening historical sqlite database")?,
        );
        let data_fetcher = Arc::new(DataFetcher::new(data_dir.clone(), historical_store.clone()));
        let fetch_state = Arc::new(tokio::sync::RwLock::new(FetchState::default()));

        let web_state = Arc::new(web::AppState {
            engine: engine.clone(),
            store: store.clone(),
            historical_store,
            shutdown_tx: shutdown_tx.clone(),
            backtest: Arc::new(tokio::sync::Mutex::new(web::BacktestState {
                status: web::BacktestRunStatus::Idle,
                progress: None,
                result: None,
                error: None,
                live_snapshot: None,
            })),
            data_dir,
            updates_tx: updates_tx.clone(),
            specimens: specimens.clone(),
            session: Arc::new(tokio::sync::RwLock::new(web::SessionState::default())),
            fetch_state,
            data_fetcher,
        });

        let router = web::build_router(web_state.clone());
        let bind_addr = app_config.web.bind_addr.clone();

        let web_state_clone = web_state.clone();
        let mut tick_rx = engine.subscribe_ticks();
        let mut decision_id_counter: i64 = 0;
        tokio::spawn(async move {
            while let Ok(tick_metrics) = tick_rx.recv().await {
                // persist and broadcast each decision
                for decision in &tick_metrics.decisions {
                    decision_id_counter += 1;

                    let side = decision.side.as_deref().and_then(|s| match s {
                        "Yes" => Some(Side::Yes),
                        "No" => Some(Side::No),
                        _ => None,
                    });
                    let action = match decision.action.as_str() {
                        "exit" => DecisionAction::Exit,
                        "skip" => DecisionAction::Skip,
                        _ => DecisionAction::Enter,
                    };

                    let core_decision = Decision {
                        id: None,
                        timestamp: decision.timestamp,
                        ticker: decision.ticker.clone(),
                        action,
                        side,
                        score: decision.score,
                        confidence: 0.0,
                        scorer_breakdown: decision.scorer_breakdown.clone(),
                        reason: decision.reason.clone(),
                        signal_id: None,
                        fill_id: None,
                        latency_ms: Some(decision.latency_ms as i64),
                    };

                    if let Err(e) = web_state_clone.store.record_decision(&core_decision).await {
                        tracing::warn!(error = %e, "failed to persist decision");
                    }

                    let decision_msg = web::ServerMessage::Decision {
                        id: decision_id_counter,
                        timestamp: decision.timestamp.to_rfc3339(),
                        ticker: decision.ticker.clone(),
                        action: decision.action.clone(),
                        side: decision.side.clone(),
                        score: decision.score,
                        confidence: 0.0,
                        scorer_breakdown: decision.scorer_breakdown.clone(),
                        reason: decision.reason.clone(),
                        fill_id: None,
                        latency_ms: Some(decision.latency_ms),
                    };
                    let _ = web_state_clone.updates_tx.send(decision_msg);
                }

                // broadcast tick update
                let pipeline_metrics = web::PipelineMetrics {
                    candidates_fetched: tick_metrics.candidates_fetched,
                    candidates_filtered: tick_metrics.candidates_filtered,
                    candidates_selected: tick_metrics.candidates_selected,
                    signals_generated: tick_metrics.signals_generated,
                    fills_executed: tick_metrics.fills_executed,
                    duration_ms: tick_metrics.duration_ms,
                };
                let update = web::ws::build_tick_update(&web_state_clone, pipeline_metrics).await;
                let _ = web_state_clone.updates_tx.send(update);
            }
        });

        info!(addr = %bind_addr, "starting web dashboard");

        match tokio::net::TcpListener::bind(&bind_addr).await {
            Ok(listener) => {
                tokio::spawn(async move {
                    if let Err(e) = axum::serve(listener, router).await {
                        tracing::error!(error = %e, "web server error");
                    }
                });
            }
            Err(e) => {
                tracing::warn!(
                    addr = %bind_addr,
                    error = %e,
                    "web dashboard disabled (port in use)"
                );
            }
        }
    }

    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("ctrl+c received, shutting down");
        let _ = shutdown_tx_clone.send(());
    });

    engine.run().await?;

    info!("paper trading session ended");
    Ok(())
}
