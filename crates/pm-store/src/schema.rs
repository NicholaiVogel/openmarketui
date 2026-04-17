//! Database schema and migrations
//!
//! SQLite schema for persisting garden state

pub const MIGRATIONS: &str = r#"
CREATE TABLE IF NOT EXISTS portfolio_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    cash TEXT NOT NULL,
    initial_capital TEXT NOT NULL,
    realized_pnl TEXT NOT NULL DEFAULT '0',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS positions (
    ticker TEXT PRIMARY KEY,
    side TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    avg_entry_price TEXT NOT NULL,
    entry_time TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS fills (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ticker TEXT NOT NULL,
    side TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    price TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    fee TEXT,
    pnl TEXT,
    exit_reason TEXT
);

CREATE TABLE IF NOT EXISTS equity_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    equity TEXT NOT NULL,
    cash TEXT NOT NULL,
    positions_value TEXT NOT NULL,
    drawdown_pct REAL NOT NULL DEFAULT 0.0
);

CREATE TABLE IF NOT EXISTS circuit_breaker_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    rule TEXT NOT NULL,
    details TEXT NOT NULL,
    action TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS pipeline_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    candidates_fetched INTEGER NOT NULL DEFAULT 0,
    candidates_filtered INTEGER NOT NULL DEFAULT 0,
    candidates_selected INTEGER NOT NULL DEFAULT 0,
    signals_generated INTEGER NOT NULL DEFAULT 0,
    fills_executed INTEGER NOT NULL DEFAULT 0,
    errors TEXT
);

CREATE TABLE IF NOT EXISTS decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    ticker TEXT NOT NULL,
    action TEXT NOT NULL,
    side TEXT,
    score REAL NOT NULL,
    confidence REAL,
    scorer_breakdown TEXT,
    reason TEXT,
    signal_id INTEGER,
    fill_id INTEGER,
    latency_ms INTEGER
);

CREATE INDEX IF NOT EXISTS idx_decisions_timestamp ON decisions(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_decisions_ticker ON decisions(ticker);

CREATE TABLE IF NOT EXISTS audit_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    actor TEXT NOT NULL,
    command TEXT NOT NULL,
    profile TEXT,
    dry_run INTEGER NOT NULL DEFAULT 0,
    request_json TEXT,
    result_json TEXT,
    trace_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_audit_events_timestamp ON audit_events(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_actor ON audit_events(actor);
CREATE INDEX IF NOT EXISTS idx_audit_events_command ON audit_events(command);

CREATE TABLE IF NOT EXISTS backtest_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL UNIQUE,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    status TEXT NOT NULL,
    start_time TEXT NOT NULL,
    end_time TEXT NOT NULL,
    capital REAL NOT NULL,
    max_positions INTEGER NOT NULL,
    max_position INTEGER NOT NULL,
    interval_hours INTEGER NOT NULL,
    kelly_fraction REAL NOT NULL,
    max_position_pct REAL NOT NULL,
    take_profit REAL NOT NULL,
    stop_loss REAL NOT NULL,
    max_hold_hours INTEGER NOT NULL,
    data_source TEXT NOT NULL,
    total_return REAL,
    total_return_pct REAL,
    sharpe_ratio REAL,
    max_drawdown_pct REAL,
    win_rate REAL,
    total_trades INTEGER,
    result_json TEXT,
    error TEXT
);

CREATE INDEX IF NOT EXISTS idx_backtest_runs_started_at ON backtest_runs(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_backtest_runs_status ON backtest_runs(status);

CREATE TABLE IF NOT EXISTS market_cache (
    ticker TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    category TEXT,
    series TEXT,
    status TEXT NOT NULL,
    yes_price REAL,
    no_price REAL,
    volume_24h REAL,
    open_interest REAL,
    close_time TEXT,
    last_updated TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_market_cache_category ON market_cache(category);
CREATE INDEX IF NOT EXISTS idx_market_cache_status ON market_cache(status);
CREATE INDEX IF NOT EXISTS idx_market_cache_volume ON market_cache(volume_24h DESC);

CREATE TABLE IF NOT EXISTS watchlist (
    ticker TEXT PRIMARY KEY,
    added_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS historical_markets (
    ticker TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    category TEXT NOT NULL,
    open_time TEXT NOT NULL,
    close_time TEXT NOT NULL,
    result TEXT
);

CREATE INDEX IF NOT EXISTS idx_hist_markets_open ON historical_markets(open_time);
CREATE INDEX IF NOT EXISTS idx_hist_markets_close ON historical_markets(close_time);

CREATE TABLE IF NOT EXISTS historical_trades (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    ticker TEXT NOT NULL,
    price TEXT NOT NULL,
    volume INTEGER NOT NULL,
    taker_side TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_hist_trades_ticker_ts ON historical_trades(ticker, timestamp);
CREATE INDEX IF NOT EXISTS idx_hist_trades_ts ON historical_trades(timestamp);
"#;
