//! Database queries for persisting garden state
//!
//! This is the root cellar - where we store everything that
//! needs to survive the winter.

use chrono::{DateTime, Utc};
use pm_core::{Decision, DecisionAction, Fill, Portfolio, Position, Side};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::str::FromStr;

use super::schema::MIGRATIONS;

/// SQLite-backed persistence store
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        let url = format!("sqlite:{}?mode=rwc", db_path);
        let pool = SqlitePool::connect(&url).await?;
        let store = Self { pool };
        store.run_migrations().await?;
        Ok(store)
    }

    async fn run_migrations(&self) -> anyhow::Result<()> {
        sqlx::raw_sql(MIGRATIONS).execute(&self.pool).await?;
        Ok(())
    }

    fn json_metric_f64(value: &serde_json::Value, key: &str) -> Option<f64> {
        value.get(key).and_then(serde_json::Value::as_f64)
    }

    fn json_metric_i64(value: &serde_json::Value, key: &str) -> Option<i64> {
        value.get(key).and_then(serde_json::Value::as_i64)
    }

    /// Load the portfolio state from the database
    pub async fn load_portfolio(&self) -> anyhow::Result<Option<Portfolio>> {
        let row = sqlx::query_as::<_, (String, String, Option<String>)>(
            "SELECT cash, initial_capital, realized_pnl FROM portfolio_state WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some((cash_str, capital_str, realized_pnl_str)) = row else {
            return Ok(None);
        };

        let cash = Decimal::from_str(&cash_str)?;
        let initial_capital = Decimal::from_str(&capital_str)?;
        let realized_pnl = realized_pnl_str
            .as_deref()
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or(Decimal::ZERO);

        let position_rows = sqlx::query_as::<_, (String, String, i64, String, String)>(
            "SELECT ticker, side, quantity, avg_entry_price, entry_time FROM positions",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut positions = HashMap::new();
        for (ticker, side_str, qty, price_str, time_str) in position_rows {
            let side = match side_str.as_str() {
                "Yes" => Side::Yes,
                _ => Side::No,
            };
            let price = Decimal::from_str(&price_str)?;
            let entry_time: DateTime<Utc> = time_str.parse::<DateTime<Utc>>()?;

            positions.insert(
                ticker.clone(),
                Position {
                    ticker: ticker.clone(),
                    title: ticker.clone(), // fallback to ticker if no title stored
                    category: String::new(),
                    side,
                    quantity: qty as u64,
                    avg_entry_price: price,
                    entry_time,
                    close_time: None,
                },
            );
        }

        Ok(Some(Portfolio {
            positions,
            cash,
            initial_capital,
            realized_pnl,
        }))
    }

    /// Save the portfolio state to the database
    pub async fn save_portfolio(&self, portfolio: &Portfolio) -> anyhow::Result<()> {
        let cash = portfolio.cash.to_string();
        let capital = portfolio.initial_capital.to_string();
        let realized_pnl = portfolio.realized_pnl.to_string();

        sqlx::query(
            "INSERT INTO portfolio_state (id, cash, initial_capital, realized_pnl, updated_at) \
             VALUES (1, ?1, ?2, ?3, datetime('now')) \
             ON CONFLICT(id) DO UPDATE SET \
             cash = ?1, initial_capital = ?2, realized_pnl = ?3, updated_at = datetime('now')",
        )
        .bind(&cash)
        .bind(&capital)
        .bind(&realized_pnl)
        .execute(&self.pool)
        .await?;

        sqlx::query("DELETE FROM positions")
            .execute(&self.pool)
            .await?;

        for pos in portfolio.positions.values() {
            let side = match pos.side {
                Side::Yes => "Yes",
                Side::No => "No",
            };
            sqlx::query(
                "INSERT INTO positions (ticker, side, quantity, avg_entry_price, entry_time) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(&pos.ticker)
            .bind(side)
            .bind(pos.quantity as i64)
            .bind(pos.avg_entry_price.to_string())
            .bind(pos.entry_time.to_rfc3339())
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Record a fill (harvest)
    pub async fn record_fill(
        &self,
        fill: &Fill,
        pnl: Option<Decimal>,
        exit_reason: Option<&str>,
    ) -> anyhow::Result<()> {
        let side = match fill.side {
            Side::Yes => "Yes",
            Side::No => "No",
        };
        sqlx::query(
            "INSERT INTO fills (ticker, side, quantity, price, timestamp, fee, pnl, exit_reason) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&fill.ticker)
        .bind(side)
        .bind(fill.quantity as i64)
        .bind(fill.price.to_string())
        .bind(fill.timestamp.to_rfc3339())
        .bind(fill.fee.map(|f| f.to_string()))
        .bind(pnl.map(|p| p.to_string()))
        .bind(exit_reason)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Snapshot current equity
    pub async fn snapshot_equity(
        &self,
        timestamp: DateTime<Utc>,
        equity: Decimal,
        cash: Decimal,
        positions_value: Decimal,
        drawdown_pct: f64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO equity_snapshots (timestamp, equity, cash, positions_value, drawdown_pct) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(timestamp.to_rfc3339())
        .bind(equity.to_string())
        .bind(cash.to_string())
        .bind(positions_value.to_string())
        .bind(drawdown_pct)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Record a circuit breaker event (frost protection trigger)
    pub async fn record_circuit_breaker_event(
        &self,
        rule: &str,
        details: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO circuit_breaker_events (timestamp, rule, details, action) \
             VALUES (datetime('now'), ?1, ?2, ?3)",
        )
        .bind(rule)
        .bind(details)
        .bind(action)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Record a pipeline run
    pub async fn record_pipeline_run(
        &self,
        timestamp: DateTime<Utc>,
        duration_ms: u64,
        candidates_fetched: usize,
        candidates_filtered: usize,
        candidates_selected: usize,
        signals_generated: usize,
        fills_executed: usize,
        errors: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO pipeline_runs \
             (timestamp, duration_ms, candidates_fetched, candidates_filtered, \
              candidates_selected, signals_generated, fills_executed, errors) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(timestamp.to_rfc3339())
        .bind(duration_ms as i64)
        .bind(candidates_fetched as i64)
        .bind(candidates_filtered as i64)
        .bind(candidates_selected as i64)
        .bind(signals_generated as i64)
        .bind(fills_executed as i64)
        .bind(errors)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get the equity curve
    pub async fn get_equity_curve(&self) -> anyhow::Result<Vec<EquitySnapshot>> {
        let rows = sqlx::query_as::<_, (String, String, String, String, f64)>(
            "SELECT timestamp, equity, cash, positions_value, drawdown_pct \
             FROM equity_snapshots ORDER BY timestamp ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut snapshots = Vec::with_capacity(rows.len());
        for (ts, eq, cash, pv, dd) in rows {
            snapshots.push(EquitySnapshot {
                timestamp: ts.parse::<DateTime<Utc>>()?,
                equity: Decimal::from_str(&eq)?,
                cash: Decimal::from_str(&cash)?,
                positions_value: Decimal::from_str(&pv)?,
                drawdown_pct: dd,
            });
        }
        Ok(snapshots)
    }

    /// Get recent fills (harvests)
    pub async fn get_recent_fills(&self, limit: u32) -> anyhow::Result<Vec<FillRecord>> {
        let rows = sqlx::query_as::<
            _,
            (
                String,
                String,
                i64,
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
            ),
        >(
            "SELECT ticker, side, quantity, price, timestamp, fee, pnl, exit_reason \
             FROM fills ORDER BY id DESC LIMIT ?1",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut fills = Vec::with_capacity(rows.len());
        for (ticker, side, qty, price, ts, fee, pnl, reason) in rows {
            fills.push(FillRecord {
                ticker,
                side: match side.as_str() {
                    "Yes" => Side::Yes,
                    _ => Side::No,
                },
                quantity: qty as u64,
                price: Decimal::from_str(&price)?,
                timestamp: ts.parse::<DateTime<Utc>>()?,
                fee: fee.map(|f| Decimal::from_str(&f)).transpose()?,
                pnl: pnl.map(|p| Decimal::from_str(&p)).transpose()?,
                exit_reason: reason,
            });
        }
        Ok(fills)
    }

    /// Count fills since a timestamp
    pub async fn get_fills_since(&self, since: DateTime<Utc>) -> anyhow::Result<u32> {
        let row = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM fills WHERE timestamp >= ?1")
            .bind(since.to_rfc3339())
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0 as u32)
    }

    /// Get circuit breaker events
    pub async fn get_circuit_breaker_events(&self, limit: u32) -> anyhow::Result<Vec<CbEvent>> {
        let rows = sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT timestamp, rule, details, action \
             FROM circuit_breaker_events ORDER BY id DESC LIMIT ?1",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut events = Vec::with_capacity(rows.len());
        for (ts, rule, details, action) in rows {
            events.push(CbEvent {
                timestamp: ts.parse::<DateTime<Utc>>()?,
                rule,
                details,
                action,
            });
        }
        Ok(events)
    }

    /// Get peak equity (for drawdown calculation)
    pub async fn get_peak_equity(&self) -> anyhow::Result<Option<Decimal>> {
        let row =
            sqlx::query_as::<_, (Option<String>,)>("SELECT MAX(equity) FROM equity_snapshots")
                .fetch_one(&self.pool)
                .await?;

        match row.0 {
            Some(s) => Ok(Some(Decimal::from_str(&s)?)),
            None => Ok(None),
        }
    }

    /// Record a decision
    pub async fn record_decision(&self, decision: &Decision) -> anyhow::Result<i64> {
        let action = decision.action.to_string();
        let side = decision.side.map(|s| match s {
            Side::Yes => "Yes",
            Side::No => "No",
        });
        let breakdown_json = serde_json::to_string(&decision.scorer_breakdown)?;

        let result = sqlx::query(
            "INSERT INTO decisions \
             (timestamp, ticker, action, side, score, confidence, scorer_breakdown, reason, signal_id, fill_id, latency_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )
        .bind(decision.timestamp.to_rfc3339())
        .bind(&decision.ticker)
        .bind(&action)
        .bind(side)
        .bind(decision.score)
        .bind(decision.confidence)
        .bind(&breakdown_json)
        .bind(&decision.reason)
        .bind(decision.signal_id)
        .bind(decision.fill_id)
        .bind(decision.latency_ms)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get recent decisions
    pub async fn get_recent_decisions(&self, limit: u32) -> anyhow::Result<Vec<DecisionRecord>> {
        let rows = sqlx::query_as::<_, DecisionRow>(
            "SELECT id, timestamp, ticker, action, side, score, confidence, scorer_breakdown, reason, signal_id, fill_id, latency_ms \
             FROM decisions ORDER BY id DESC LIMIT ?1",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut decisions = Vec::with_capacity(rows.len());
        for row in rows {
            let action = row
                .action
                .parse::<DecisionAction>()
                .unwrap_or(DecisionAction::Skip);
            let side = row.side.as_deref().map(|s| match s {
                "Yes" => Side::Yes,
                _ => Side::No,
            });
            let scorer_breakdown: HashMap<String, f64> = row
                .scorer_breakdown
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();

            decisions.push(DecisionRecord {
                id: row.id,
                timestamp: row.timestamp.parse::<DateTime<Utc>>()?,
                ticker: row.ticker,
                action,
                side,
                score: row.score,
                confidence: row.confidence.unwrap_or(0.0),
                scorer_breakdown,
                reason: row.reason,
                signal_id: row.signal_id,
                fill_id: row.fill_id,
                latency_ms: row.latency_ms,
            });
        }
        Ok(decisions)
    }

    /// Get a single decision by ID
    pub async fn get_decision(&self, id: i64) -> anyhow::Result<Option<DecisionRecord>> {
        let row = sqlx::query_as::<_, DecisionRow>(
            "SELECT id, timestamp, ticker, action, side, score, confidence, scorer_breakdown, reason, signal_id, fill_id, latency_ms \
             FROM decisions WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let action = row
                    .action
                    .parse::<DecisionAction>()
                    .unwrap_or(DecisionAction::Skip);
                let side = row.side.as_deref().map(|s| match s {
                    "Yes" => Side::Yes,
                    _ => Side::No,
                });
                let scorer_breakdown: HashMap<String, f64> = row
                    .scorer_breakdown
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();

                Ok(Some(DecisionRecord {
                    id: row.id,
                    timestamp: row.timestamp.parse::<DateTime<Utc>>()?,
                    ticker: row.ticker,
                    action,
                    side,
                    score: row.score,
                    confidence: row.confidence.unwrap_or(0.0),
                    scorer_breakdown,
                    reason: row.reason,
                    signal_id: row.signal_id,
                    fill_id: row.fill_id,
                    latency_ms: row.latency_ms,
                }))
            }
            None => Ok(None),
        }
    }

    /// Record an audit event
    pub async fn record_audit_event(&self, event: &NewAuditEvent) -> anyhow::Result<i64> {
        let request_json = event
            .request
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let result_json = event
            .result
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        let result = sqlx::query(
            "INSERT INTO audit_events \
             (timestamp, actor, command, profile, dry_run, request_json, result_json, trace_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(&event.actor)
        .bind(&event.command)
        .bind(&event.profile)
        .bind(if event.dry_run { 1_i64 } else { 0_i64 })
        .bind(request_json)
        .bind(result_json)
        .bind(&event.trace_id)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get recent audit events
    pub async fn get_recent_audit_events(&self, limit: u32) -> anyhow::Result<Vec<AuditEvent>> {
        let rows = sqlx::query_as::<_, AuditEventRow>(
            "SELECT id, timestamp, actor, command, profile, dry_run, request_json, result_json, trace_id \
             FROM audit_events ORDER BY id DESC LIMIT ?1",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(AuditEvent::try_from).collect()
    }

    /// Record a backtest run when the daemon accepts it.
    pub async fn record_backtest_run_started(&self, run: &NewBacktestRun) -> anyhow::Result<i64> {
        let result = sqlx::query(
            "INSERT INTO backtest_runs \
             (run_id, started_at, status, start_time, end_time, capital, max_positions, \
              max_position, interval_hours, kelly_fraction, max_position_pct, take_profit, \
              stop_loss, max_hold_hours, data_source) \
             VALUES (?1, ?2, 'running', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        )
        .bind(&run.run_id)
        .bind(run.started_at.to_rfc3339())
        .bind(run.start_time.to_rfc3339())
        .bind(run.end_time.to_rfc3339())
        .bind(run.capital)
        .bind(run.max_positions as i64)
        .bind(run.max_position as i64)
        .bind(run.interval_hours)
        .bind(run.kelly_fraction)
        .bind(run.max_position_pct)
        .bind(run.take_profit)
        .bind(run.stop_loss)
        .bind(run.max_hold_hours)
        .bind(&run.data_source)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Mark a backtest run as complete and store its result payload.
    pub async fn complete_backtest_run(
        &self,
        run_id: &str,
        result: &serde_json::Value,
    ) -> anyhow::Result<()> {
        let result_json = serde_json::to_string(result)?;
        sqlx::query(
            "UPDATE backtest_runs SET \
             completed_at = ?2, status = 'complete', total_return = ?3, total_return_pct = ?4, \
             sharpe_ratio = ?5, max_drawdown_pct = ?6, win_rate = ?7, total_trades = ?8, \
             result_json = ?9, error = NULL \
             WHERE run_id = ?1",
        )
        .bind(run_id)
        .bind(Utc::now().to_rfc3339())
        .bind(Self::json_metric_f64(result, "total_return"))
        .bind(Self::json_metric_f64(result, "total_return_pct"))
        .bind(Self::json_metric_f64(result, "sharpe_ratio"))
        .bind(Self::json_metric_f64(result, "max_drawdown_pct"))
        .bind(Self::json_metric_f64(result, "win_rate"))
        .bind(Self::json_metric_i64(result, "total_trades"))
        .bind(result_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark a backtest run as failed or stopped.
    pub async fn finish_backtest_run_with_error(
        &self,
        run_id: &str,
        status: &str,
        error: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE backtest_runs SET completed_at = ?2, status = ?3, error = ?4 WHERE run_id = ?1",
        )
        .bind(run_id)
        .bind(Utc::now().to_rfc3339())
        .bind(status)
        .bind(error)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get recent backtest runs, newest first.
    pub async fn get_recent_backtest_runs(
        &self,
        limit: u32,
    ) -> anyhow::Result<Vec<BacktestRunRecord>> {
        let rows = sqlx::query_as::<_, BacktestRunRow>(
            "SELECT id, run_id, started_at, completed_at, status, start_time, end_time, capital, \
             max_positions, max_position, interval_hours, kelly_fraction, max_position_pct, \
             take_profit, stop_loss, max_hold_hours, data_source, total_return, total_return_pct, \
             sharpe_ratio, max_drawdown_pct, win_rate, total_trades, result_json, error \
             FROM backtest_runs ORDER BY id DESC LIMIT ?1",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(BacktestRunRecord::try_from).collect()
    }

    /// Get a backtest run by numeric ID or daemon run ID.
    pub async fn get_backtest_run(&self, key: &str) -> anyhow::Result<Option<BacktestRunRecord>> {
        let row = if let Ok(id) = key.parse::<i64>() {
            sqlx::query_as::<_, BacktestRunRow>(
                "SELECT id, run_id, started_at, completed_at, status, start_time, end_time, capital, \
                 max_positions, max_position, interval_hours, kelly_fraction, max_position_pct, \
                 take_profit, stop_loss, max_hold_hours, data_source, total_return, total_return_pct, \
                 sharpe_ratio, max_drawdown_pct, win_rate, total_trades, result_json, error \
                 FROM backtest_runs WHERE id = ?1",
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, BacktestRunRow>(
                "SELECT id, run_id, started_at, completed_at, status, start_time, end_time, capital, \
                 max_positions, max_position, interval_hours, kelly_fraction, max_position_pct, \
                 take_profit, stop_loss, max_hold_hours, data_source, total_return, total_return_pct, \
                 sharpe_ratio, max_drawdown_pct, win_rate, total_trades, result_json, error \
                 FROM backtest_runs WHERE run_id = ?1",
            )
            .bind(key)
            .fetch_optional(&self.pool)
            .await?
        };

        row.map(BacktestRunRecord::try_from).transpose()
    }

    /// Get decisions for a specific ticker
    pub async fn get_decisions_for_ticker(
        &self,
        ticker: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<DecisionRecord>> {
        let rows = sqlx::query_as::<_, DecisionRow>(
            "SELECT id, timestamp, ticker, action, side, score, confidence, scorer_breakdown, reason, signal_id, fill_id, latency_ms \
             FROM decisions WHERE ticker = ?1 ORDER BY id DESC LIMIT ?2",
        )
        .bind(ticker)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut decisions = Vec::with_capacity(rows.len());
        for row in rows {
            let action = row
                .action
                .parse::<DecisionAction>()
                .unwrap_or(DecisionAction::Skip);
            let side = row.side.as_deref().map(|s| match s {
                "Yes" => Side::Yes,
                _ => Side::No,
            });
            let scorer_breakdown: HashMap<String, f64> = row
                .scorer_breakdown
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();

            decisions.push(DecisionRecord {
                id: row.id,
                timestamp: row.timestamp.parse::<DateTime<Utc>>()?,
                ticker: row.ticker,
                action,
                side,
                score: row.score,
                confidence: row.confidence.unwrap_or(0.0),
                scorer_breakdown,
                reason: row.reason,
                signal_id: row.signal_id,
                fill_id: row.fill_id,
                latency_ms: row.latency_ms,
            });
        }
        Ok(decisions)
    }

    /// Upsert a market into the cache
    pub async fn upsert_market(&self, market: &MarketCacheEntry) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO market_cache \
             (ticker, title, category, series, status, yes_price, no_price, volume_24h, open_interest, close_time, last_updated) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
             ON CONFLICT(ticker) DO UPDATE SET \
             title = ?2, category = ?3, series = ?4, status = ?5, yes_price = ?6, no_price = ?7, \
             volume_24h = ?8, open_interest = ?9, close_time = ?10, last_updated = ?11",
        )
        .bind(&market.ticker)
        .bind(&market.title)
        .bind(&market.category)
        .bind(&market.series)
        .bind(&market.status)
        .bind(market.yes_price)
        .bind(market.no_price)
        .bind(market.volume_24h)
        .bind(market.open_interest)
        .bind(market.close_time.map(|t| t.to_rfc3339()))
        .bind(market.last_updated.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get markets from cache with optional filtering
    pub async fn get_markets(
        &self,
        category: Option<&str>,
        status: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> anyhow::Result<Vec<MarketCacheEntry>> {
        let mut query = String::from(
            "SELECT ticker, title, category, series, status, yes_price, no_price, \
             volume_24h, open_interest, close_time, last_updated FROM market_cache WHERE 1=1",
        );

        if category.is_some() {
            query.push_str(" AND category = ?1");
        }
        if status.is_some() {
            query.push_str(" AND status = ?2");
        }
        query.push_str(" ORDER BY volume_24h DESC NULLS LAST LIMIT ?3 OFFSET ?4");

        let mut q = sqlx::query_as::<_, MarketCacheRow>(&query);
        if let Some(cat) = category {
            q = q.bind(cat);
        }
        if let Some(st) = status {
            q = q.bind(st);
        }
        q = q.bind(limit as i64).bind(offset as i64);

        let rows = q.fetch_all(&self.pool).await?;

        let mut markets = Vec::with_capacity(rows.len());
        for row in rows {
            markets.push(MarketCacheEntry {
                ticker: row.ticker,
                title: row.title,
                category: row.category,
                series: row.series,
                status: row.status,
                yes_price: row.yes_price,
                no_price: row.no_price,
                volume_24h: row.volume_24h,
                open_interest: row.open_interest,
                close_time: row.close_time.and_then(|s| s.parse::<DateTime<Utc>>().ok()),
                last_updated: row.last_updated.parse::<DateTime<Utc>>()?,
            });
        }
        Ok(markets)
    }

    /// Search markets by title
    pub async fn search_markets(
        &self,
        query: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<MarketCacheEntry>> {
        let pattern = format!("%{}%", query);
        let rows = sqlx::query_as::<_, MarketCacheRow>(
            "SELECT ticker, title, category, series, status, yes_price, no_price, \
             volume_24h, open_interest, close_time, last_updated FROM market_cache \
             WHERE title LIKE ?1 OR ticker LIKE ?1 \
             ORDER BY volume_24h DESC NULLS LAST LIMIT ?2",
        )
        .bind(&pattern)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut markets = Vec::with_capacity(rows.len());
        for row in rows {
            markets.push(MarketCacheEntry {
                ticker: row.ticker,
                title: row.title,
                category: row.category,
                series: row.series,
                status: row.status,
                yes_price: row.yes_price,
                no_price: row.no_price,
                volume_24h: row.volume_24h,
                open_interest: row.open_interest,
                close_time: row.close_time.and_then(|s| s.parse::<DateTime<Utc>>().ok()),
                last_updated: row.last_updated.parse::<DateTime<Utc>>()?,
            });
        }
        Ok(markets)
    }

    /// Get a single market from cache
    pub async fn get_market(&self, ticker: &str) -> anyhow::Result<Option<MarketCacheEntry>> {
        let row = sqlx::query_as::<_, MarketCacheRow>(
            "SELECT ticker, title, category, series, status, yes_price, no_price, \
             volume_24h, open_interest, close_time, last_updated FROM market_cache \
             WHERE ticker = ?1",
        )
        .bind(ticker)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(MarketCacheEntry {
                ticker: row.ticker,
                title: row.title,
                category: row.category,
                series: row.series,
                status: row.status,
                yes_price: row.yes_price,
                no_price: row.no_price,
                volume_24h: row.volume_24h,
                open_interest: row.open_interest,
                close_time: row.close_time.and_then(|s| s.parse::<DateTime<Utc>>().ok()),
                last_updated: row.last_updated.parse::<DateTime<Utc>>()?,
            })),
            None => Ok(None),
        }
    }

    /// Add a ticker to the watchlist
    pub async fn add_to_watchlist(&self, ticker: &str) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO watchlist (ticker, added_at) VALUES (?1, datetime('now'))",
        )
        .bind(ticker)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Remove a ticker from the watchlist
    pub async fn remove_from_watchlist(&self, ticker: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM watchlist WHERE ticker = ?1")
            .bind(ticker)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get the full watchlist
    pub async fn get_watchlist(&self) -> anyhow::Result<Vec<WatchlistEntry>> {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT ticker, added_at FROM watchlist ORDER BY added_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut entries = Vec::with_capacity(rows.len());
        for (ticker, added_at) in rows {
            entries.push(WatchlistEntry {
                ticker,
                added_at: added_at.parse::<DateTime<Utc>>()?,
            });
        }
        Ok(entries)
    }

    /// Check if a ticker is in the watchlist
    pub async fn is_in_watchlist(&self, ticker: &str) -> anyhow::Result<bool> {
        let row = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM watchlist WHERE ticker = ?1")
            .bind(ticker)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0 > 0)
    }

    // === historical data ingest ===

    /// Begin a transaction for bulk ingest
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Upsert a historical market record
    pub async fn upsert_historical_market(
        &self,
        ticker: &str,
        title: &str,
        category: &str,
        open_time: &str,
        close_time: &str,
        result: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO historical_markets (ticker, title, category, open_time, close_time, result) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT(ticker) DO UPDATE SET \
             title = ?2, category = ?3, open_time = ?4, close_time = ?5, result = ?6",
        )
        .bind(ticker)
        .bind(title)
        .bind(category)
        .bind(open_time)
        .bind(close_time)
        .bind(result)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Upsert historical market records in a single transaction
    pub async fn upsert_historical_markets_batch(
        &self,
        markets: &[(String, String, String, String, String, Option<String>)],
    ) -> anyhow::Result<()> {
        if markets.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;
        for (ticker, title, category, open_time, close_time, result) in markets {
            sqlx::query(
                "INSERT INTO historical_markets (ticker, title, category, open_time, close_time, result) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT(ticker) DO UPDATE SET \
                 title = ?2, category = ?3, open_time = ?4, close_time = ?5, result = ?6",
            )
            .bind(ticker)
            .bind(title)
            .bind(category)
            .bind(open_time)
            .bind(close_time)
            .bind(result)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Insert a batch of historical trades using a transaction
    pub async fn insert_historical_trades_batch(
        &self,
        trades: &[(String, String, String, i64, String)], // (timestamp, ticker, price, volume, side)
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        for (ts, ticker, price, volume, side) in trades {
            sqlx::query(
                "INSERT INTO historical_trades (timestamp, ticker, price, volume, taker_side) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(ts)
            .bind(ticker)
            .bind(price)
            .bind(*volume)
            .bind(side)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Clear all historical trades (for re-ingest)
    pub async fn clear_historical_trades(&self) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM historical_trades")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Clear all historical markets (for re-ingest)
    pub async fn clear_historical_markets(&self) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM historical_markets")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Count historical markets
    pub async fn count_historical_markets(&self) -> anyhow::Result<i64> {
        let row = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM historical_markets")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Count historical trades
    pub async fn count_historical_trades(&self) -> anyhow::Result<i64> {
        let row = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM historical_trades")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Get summary stats for historical trades (min timestamp, max timestamp, distinct day count)
    pub async fn get_historical_trades_summary(
        &self,
    ) -> anyhow::Result<Option<(String, String, i64)>> {
        let row: Option<(Option<String>, Option<String>, i64)> = sqlx::query_as(
            "SELECT MIN(timestamp), MAX(timestamp), COUNT(DISTINCT DATE(timestamp)) FROM historical_trades",
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some((Some(min_ts), Some(max_ts), days)) => Ok(Some((min_ts, max_ts, days))),
            _ => Ok(None),
        }
    }

    // === historical data queries (for backtesting) ===

    /// Get all markets active during a time range
    pub async fn get_historical_markets_in_range(
        &self,
        start: &str,
        end: &str,
    ) -> anyhow::Result<Vec<HistoricalMarketRow>> {
        let rows = sqlx::query_as::<_, HistoricalMarketRow>(
            "SELECT ticker, title, category, open_time, close_time, result \
             FROM historical_markets \
             WHERE open_time < ?2 AND close_time > ?1",
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get all trades for a time range
    pub async fn get_historical_trades_in_range(
        &self,
        start: &str,
        end: &str,
    ) -> anyhow::Result<Vec<HistoricalTradeRow>> {
        let rows = sqlx::query_as::<_, HistoricalTradeRow>(
            "SELECT timestamp, ticker, price, volume, taker_side \
             FROM historical_trades \
             WHERE timestamp >= ?1 AND timestamp < ?2 \
             ORDER BY timestamp ASC",
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get distinct traded tickers for a date range (inclusive)
    pub async fn get_historical_trade_tickers_in_range(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>(
            "SELECT DISTINCT ticker FROM historical_trades \
             WHERE DATE(timestamp) >= DATE(?1) AND DATE(timestamp) <= DATE(?2)",
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|(ticker,)| ticker).collect())
    }

    /// Get all known historical market tickers
    pub async fn get_historical_market_tickers(&self) -> anyhow::Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>("SELECT ticker FROM historical_markets")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|(ticker,)| ticker).collect())
    }
}

/// A point on the equity curve
#[derive(Debug, Clone)]
pub struct EquitySnapshot {
    pub timestamp: DateTime<Utc>,
    pub equity: Decimal,
    pub cash: Decimal,
    pub positions_value: Decimal,
    pub drawdown_pct: f64,
}

/// A recorded fill (harvest record)
#[derive(Debug, Clone)]
pub struct FillRecord {
    pub ticker: String,
    pub side: Side,
    pub quantity: u64,
    pub price: Decimal,
    pub timestamp: DateTime<Utc>,
    pub fee: Option<Decimal>,
    pub pnl: Option<Decimal>,
    pub exit_reason: Option<String>,
}

/// A circuit breaker event (frost protection event)
#[derive(Debug, Clone)]
pub struct CbEvent {
    pub timestamp: DateTime<Utc>,
    pub rule: String,
    pub details: String,
    pub action: String,
}

/// Internal row type for SQLx queries
#[derive(Debug, sqlx::FromRow)]
struct DecisionRow {
    id: i64,
    timestamp: String,
    ticker: String,
    action: String,
    side: Option<String>,
    score: f64,
    confidence: Option<f64>,
    scorer_breakdown: Option<String>,
    reason: Option<String>,
    signal_id: Option<i64>,
    fill_id: Option<i64>,
    latency_ms: Option<i64>,
}

/// A recorded decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub ticker: String,
    pub action: DecisionAction,
    pub side: Option<Side>,
    pub score: f64,
    pub confidence: f64,
    pub scorer_breakdown: HashMap<String, f64>,
    pub reason: Option<String>,
    pub signal_id: Option<i64>,
    pub fill_id: Option<i64>,
    pub latency_ms: Option<i64>,
}

/// An audit event to record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewAuditEvent {
    pub actor: String,
    pub command: String,
    pub profile: Option<String>,
    pub dry_run: bool,
    pub request: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub trace_id: Option<String>,
}

/// Internal row type for audit queries
#[derive(Debug, sqlx::FromRow)]
struct AuditEventRow {
    id: i64,
    timestamp: String,
    actor: String,
    command: String,
    profile: Option<String>,
    dry_run: i64,
    request_json: Option<String>,
    result_json: Option<String>,
    trace_id: Option<String>,
}

/// A recorded audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub actor: String,
    pub command: String,
    pub profile: Option<String>,
    pub dry_run: bool,
    pub request: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub trace_id: Option<String>,
}

impl TryFrom<AuditEventRow> for AuditEvent {
    type Error = anyhow::Error;

    fn try_from(row: AuditEventRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            timestamp: row.timestamp.parse::<DateTime<Utc>>()?,
            actor: row.actor,
            command: row.command,
            profile: row.profile,
            dry_run: row.dry_run != 0,
            request: row
                .request_json
                .as_deref()
                .map(serde_json::from_str)
                .transpose()?,
            result: row
                .result_json
                .as_deref()
                .map(serde_json::from_str)
                .transpose()?,
            trace_id: row.trace_id,
        })
    }
}

/// A backtest run accepted by the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewBacktestRun {
    pub run_id: String,
    pub started_at: DateTime<Utc>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub capital: f64,
    pub max_positions: usize,
    pub max_position: u64,
    pub interval_hours: i64,
    pub kelly_fraction: f64,
    pub max_position_pct: f64,
    pub take_profit: f64,
    pub stop_loss: f64,
    pub max_hold_hours: i64,
    pub data_source: String,
}

#[derive(Debug, sqlx::FromRow)]
struct BacktestRunRow {
    id: i64,
    run_id: String,
    started_at: String,
    completed_at: Option<String>,
    status: String,
    start_time: String,
    end_time: String,
    capital: f64,
    max_positions: i64,
    max_position: i64,
    interval_hours: i64,
    kelly_fraction: f64,
    max_position_pct: f64,
    take_profit: f64,
    stop_loss: f64,
    max_hold_hours: i64,
    data_source: String,
    total_return: Option<f64>,
    total_return_pct: Option<f64>,
    sharpe_ratio: Option<f64>,
    max_drawdown_pct: Option<f64>,
    win_rate: Option<f64>,
    total_trades: Option<i64>,
    result_json: Option<String>,
    error: Option<String>,
}

/// A durable backtest run record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestRunRecord {
    pub id: i64,
    pub run_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub capital: f64,
    pub max_positions: usize,
    pub max_position: u64,
    pub interval_hours: i64,
    pub kelly_fraction: f64,
    pub max_position_pct: f64,
    pub take_profit: f64,
    pub stop_loss: f64,
    pub max_hold_hours: i64,
    pub data_source: String,
    pub total_return: Option<f64>,
    pub total_return_pct: Option<f64>,
    pub sharpe_ratio: Option<f64>,
    pub max_drawdown_pct: Option<f64>,
    pub win_rate: Option<f64>,
    pub total_trades: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

impl TryFrom<BacktestRunRow> for BacktestRunRecord {
    type Error = anyhow::Error;

    fn try_from(row: BacktestRunRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            run_id: row.run_id,
            started_at: row.started_at.parse::<DateTime<Utc>>()?,
            completed_at: row
                .completed_at
                .as_deref()
                .map(str::parse::<DateTime<Utc>>)
                .transpose()?,
            status: row.status,
            start_time: row.start_time.parse::<DateTime<Utc>>()?,
            end_time: row.end_time.parse::<DateTime<Utc>>()?,
            capital: row.capital,
            max_positions: row.max_positions as usize,
            max_position: row.max_position as u64,
            interval_hours: row.interval_hours,
            kelly_fraction: row.kelly_fraction,
            max_position_pct: row.max_position_pct,
            take_profit: row.take_profit,
            stop_loss: row.stop_loss,
            max_hold_hours: row.max_hold_hours,
            data_source: row.data_source,
            total_return: row.total_return,
            total_return_pct: row.total_return_pct,
            sharpe_ratio: row.sharpe_ratio,
            max_drawdown_pct: row.max_drawdown_pct,
            win_rate: row.win_rate,
            total_trades: row.total_trades,
            result: row
                .result_json
                .as_deref()
                .map(serde_json::from_str)
                .transpose()?,
            error: row.error,
        })
    }
}

/// A cached market entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCacheEntry {
    pub ticker: String,
    pub title: String,
    pub category: Option<String>,
    pub series: Option<String>,
    pub status: String,
    pub yes_price: Option<f64>,
    pub no_price: Option<f64>,
    pub volume_24h: Option<f64>,
    pub open_interest: Option<f64>,
    pub close_time: Option<DateTime<Utc>>,
    pub last_updated: DateTime<Utc>,
}

/// Internal row type for market cache queries
#[derive(Debug, sqlx::FromRow)]
struct MarketCacheRow {
    ticker: String,
    title: String,
    category: Option<String>,
    series: Option<String>,
    status: String,
    yes_price: Option<f64>,
    no_price: Option<f64>,
    volume_24h: Option<f64>,
    open_interest: Option<f64>,
    close_time: Option<String>,
    last_updated: String,
}

/// A watchlist entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchlistEntry {
    pub ticker: String,
    pub added_at: DateTime<Utc>,
}

/// A historical market row from sqlite
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct HistoricalMarketRow {
    pub ticker: String,
    pub title: String,
    pub category: String,
    pub open_time: String,
    pub close_time: String,
    pub result: Option<String>,
}

/// A historical trade row from sqlite
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct HistoricalTradeRow {
    pub timestamp: String,
    pub ticker: String,
    pub price: String,
    pub volume: i64,
    pub taker_side: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn persists_backtest_run_lifecycle() {
        let file = NamedTempFile::new().expect("temp db");
        let store = SqliteStore::new(file.path().to_str().expect("utf8 path"))
            .await
            .expect("store opens");

        let started_at = Utc::now();
        let run = NewBacktestRun {
            run_id: "run-test-1".to_string(),
            started_at,
            start_time: started_at,
            end_time: started_at + chrono::TimeDelta::hours(24),
            capital: 10_000.0,
            max_positions: 25,
            max_position: 100,
            interval_hours: 1,
            kelly_fraction: 0.4,
            max_position_pct: 0.1,
            take_profit: 0.5,
            stop_loss: 0.99,
            max_hold_hours: 48,
            data_source: "sqlite".to_string(),
        };

        let id = store
            .record_backtest_run_started(&run)
            .await
            .expect("run start records");
        assert_eq!(id, 1);

        let running = store
            .get_backtest_run("run-test-1")
            .await
            .expect("run fetch works")
            .expect("run exists");
        assert_eq!(running.status, "running");
        assert_eq!(running.run_id, "run-test-1");
        assert_eq!(running.max_positions, 25);

        let result = json!({
            "total_return": 123.45,
            "total_return_pct": 1.23,
            "sharpe_ratio": 2.5,
            "max_drawdown_pct": 0.4,
            "win_rate": 55.0,
            "total_trades": 42
        });
        store
            .complete_backtest_run("run-test-1", &result)
            .await
            .expect("run completes");

        let completed = store
            .get_backtest_run("1")
            .await
            .expect("run fetch by id works")
            .expect("run exists by id");
        assert_eq!(completed.status, "complete");
        assert_eq!(completed.total_return, Some(123.45));
        assert_eq!(completed.total_trades, Some(42));
        assert_eq!(completed.result.as_ref(), Some(&result));

        let recent = store
            .get_recent_backtest_runs(10)
            .await
            .expect("recent runs fetch");
        assert_eq!(recent.len(), 1);
    }
}
