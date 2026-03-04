# Core Types Reference

All core types are defined in `crates/pm-core/src/`. This is the vocabulary of the entire system — every other crate depends on these types.

---

## MarketCandidate

The primary data structure flowing through the pipeline (`crates/pm-core/src/types.rs`).

```rust
pub struct MarketCandidate {
    pub ticker: String,
    pub title: String,
    pub category: String,
    pub current_yes_price: Decimal,
    pub current_no_price: Decimal,
    pub volume_24h: Decimal,
    pub price_history: Vec<PricePoint>,
    pub scores: HashMap<String, f64>,
    pub final_score: f64,
    pub close_time: Option<DateTime<Utc>>,
}
```

**`ticker`**: Unique market identifier from Kalshi (e.g., `"KXINFL-24-T3.00"`).

**`current_yes_price`**: Current ask price for the Yes side, expressed as a decimal between 0 and 1. A value of 0.35 means Yes contracts cost $0.35 each and pay $1.00 on resolution.

**`current_no_price`**: Always `1 - current_yes_price` for binary markets. Present as a field for convenience.

**`volume_24h`**: Total contract volume traded in the last 24 hours.

**`price_history`**: Ordered list of recent price observations. The lookback window is set by the source (default 24 points for the historical source). Used by trend-based scorers.

**`scores`**: Accumulated scorer outputs. Each scorer adds one or more keys. Values have no enforced range but conventionally stay within [-1.0, 1.0]. Example after a full pipeline run: `{"momentum_6h": 0.42, "volume_ratio": 0.18, "time_decay": 0.91, "final_score": 0.37}`.

**`final_score`**: The combined signal score. Positive means the pipeline sees bullish edge (favor entering); negative means bearish. Zero means no signal. The selector and executor use this field — other score keys are for introspection only.

**`close_time`**: When the market resolves, if known. Used by `TimeDecayScorer` and `TimeToCloseFilter`.

---

## PricePoint

```rust
pub struct PricePoint {
    pub timestamp: DateTime<Utc>,
    pub yes_price: Decimal,
    pub volume: Option<Decimal>,
}
```

A single historical price observation. `volume` may be absent for older historical data.

---

## TradingContext

Passed to every pipeline stage. Read-only within a pipeline execution.

```rust
pub struct TradingContext {
    pub timestamp: DateTime<Utc>,
    pub portfolio: Portfolio,
    pub trading_history: Vec<Trade>,
    pub request_id: String,
}
```

**`timestamp`**: The current logical time. In backtesting, this is the simulation clock. In live/paper trading, this is wall clock time.

**`portfolio`**: Current positions and cash. Stages read this to check for existing positions (`portfolio.has_position(ticker)`).

**`trading_history`**: Chronological list of all trades in this session.

**`request_id`**: UUID assigned per tick, for correlating log lines.

---

## Portfolio

```rust
pub struct Portfolio {
    pub cash: Decimal,
    pub initial_capital: Decimal,
    pub positions: HashMap<String, Position>,
    pub realized_pnl: Decimal,
}
```

**Methods**:
- `has_position(ticker)` → bool
- `get_position(ticker)` → `Option<&Position>`
- `apply_fill(fill)` — deducts cash and opens/adds to a position
- `close_position(ticker, exit_price)` → `Option<Decimal>` (realized P&L)
- `resolve_position(ticker, result)` → `Option<Decimal>` — settles at $1.00 or $0.00
- `total_value(prices)` → cash + mark-to-market value of all positions

---

## Position

```rust
pub struct Position {
    pub ticker: String,
    pub side: Side,
    pub quantity: u64,
    pub avg_entry_price: Decimal,
    pub entry_time: DateTime<Utc>,
}
```

Represents a single open position. `avg_entry_price` is updated if you add to an existing position (cost averaging).

---

## Side

```rust
pub enum Side {
    Yes,
    No,
}
```

Kalshi markets are binary. Holding `Yes` at 0.30 means you paid $0.30 per contract and win $1.00 if the market resolves Yes. Holding `No` at 0.70 (= 1 - 0.30) means you paid $0.70 per contract and win $1.00 if the market resolves No.

---

## Signal

```rust
pub struct Signal {
    pub ticker: String,
    pub side: Side,
    pub quantity: u64,
    pub limit_price: Option<Decimal>,
    pub reason: String,
}
```

An instruction to enter a position. The executor receives signals and returns `Fill` values. `limit_price` is advisory in backtesting (enforced with 5% tolerance) and passed to the paper executor.

---

## Fill

```rust
pub struct Fill {
    pub ticker: String,
    pub side: Side,
    pub quantity: u64,
    pub price: Decimal,
    pub timestamp: DateTime<Utc>,
    pub fee: Option<Decimal>,
}
```

Confirmation that a signal was executed. `price` is the actual fill price (may differ from signal's limit_price due to slippage). `fee` is populated by `PaperExecutor` but not by `BacktestExecutor`.

---

## ExitConfig

```rust
pub struct ExitConfig {
    pub take_profit_pct: f64,           // exit when unrealized gain ≥ this
    pub stop_loss_pct: f64,             // exit when unrealized loss ≥ this
    pub max_hold_hours: i64,            // exit after holding this long
    pub score_reversal_threshold: f64,  // exit when final_score drops below this
}
```

Preset constructors:

| Method | take_profit | stop_loss | max_hold_hours | score_reversal |
|---|---|---|---|---|
| `default()` | 0.50 | 0.99 (off) | 48 | -0.5 |
| `conservative()` | 0.15 | 0.10 | 48 | -0.2 |
| `aggressive()` | 0.30 | 0.20 | 120 | -0.5 |
| `prediction_market()` | 1.00 | 0.99 (off) | 48 | -0.5 |

Note: `stop_loss_pct = 0.99` is effectively disabled. Stop losses on binary prediction markets tend not to help because prices can gap through a stop threshold between observation windows. The default relies on score reversal and the time stop instead.

---

## ExitReason

```rust
pub enum ExitReason {
    Resolution(MarketResult),
    TakeProfit { pnl_pct: f64 },
    StopLoss { pnl_pct: f64 },
    TimeStop { hours_held: i64 },
    ScoreReversal { new_score: f64 },
}
```

Recorded on closed trades. `Resolution` is used when the market itself resolves, not when an exit rule fires.

---

## MarketResult

```rust
pub enum MarketResult {
    Yes,
    No,
    Cancelled,
}
```

`Cancelled` markets are settled at entry price — no gain, no loss.

---

## Decision

```rust
pub struct Decision {
    pub id: Option<i64>,
    pub timestamp: DateTime<Utc>,
    pub ticker: String,
    pub action: DecisionAction,        // Enter / Exit / Skip
    pub side: Option<Side>,
    pub score: f64,
    pub confidence: f64,
    pub scorer_breakdown: HashMap<String, f64>,
    pub reason: String,
    pub signal_id: Option<i64>,
    pub fill_id: Option<i64>,
    pub latency_ms: Option<i64>,
}
```

Persisted to the `decisions` table in SQLite and broadcast via WebSocket to Watchtower's decision feed. `scorer_breakdown` is a snapshot of `candidate.scores` at decision time, enabling per-scorer attribution in the UI.

---

## BacktestConfig

```rust
pub struct BacktestConfig {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub interval: TimeDelta,
    pub initial_capital: Decimal,
    pub max_position_size: u64,
    pub max_positions: usize,
}
```

Passed to `Backtester::new` or `Backtester::with_configs`. `interval` is the step size (typically `TimeDelta::hours(1)`).
