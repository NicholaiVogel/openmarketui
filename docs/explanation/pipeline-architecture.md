# Pipeline Architecture and Data Flow

This document describes how data flows through the trading pipeline, what each stage is responsible for, and how the stages compose.

## The pipeline shape

Every trading decision in OpenMarketUI passes through five ordered stages:

```
Source → Filter → Scorer → Selector → OrderExecutor
```

Each stage is defined by a trait in `crates/pm-core/src/traits.rs`. The stages are stateless transformations on a `Vec<MarketCandidate>` — each stage receives candidates, transforms them, and passes them forward.

The pipeline itself is orchestrated by `TradingPipeline` in `crates/pm-kalshi/src/pipeline/trading_pipeline.rs`. The `PipelineResult` struct captures a snapshot of candidates at each stage for observability.

## Stage-by-stage breakdown

### Source

```rust
async fn get_candidates(&self, context: &TradingContext) -> Result<Vec<MarketCandidate>, String>
```

The source produces the initial set of `MarketCandidate` values. In live and paper trading, this is `LiveKalshiSource`, which calls the Kalshi API. In backtesting, it's `HistoricalMarketSource`, which queries pre-loaded historical data filtered to a time window.

Candidates at this stage have prices and volume but no scores. The `scores` HashMap is empty.

### Filter

```rust
async fn filter(&self, context: &TradingContext, candidates: Vec<MarketCandidate>) -> Result<FilterResult, String>
```

Filters are the immune system. Each filter takes the full candidate list and returns a `FilterResult` with two buckets: `kept` and `removed`. Filters run sequentially — each filter receives only the candidates that survived the previous one.

The `FilterResult.removed` field exists for observability (visible in Watchtower as filtered counts) but doesn't affect the pipeline — only `kept` passes forward.

Filters should not score or rank — they make binary keep/remove decisions. If you find yourself writing a filter that computes a value to compare, it's probably a scorer in disguise.

Default filters in backtesting:
- `LiquidityFilter(min_volume=10)` — removes illiquid markets
- `TimeToCloseFilter(min_hours=0, max_hours=None)` — removes markets outside the time window
- `AlreadyPositionedFilter` — removes markets where we already hold a position

Default filters in paper trading:
- `LiquidityFilter(min_volume=100)`
- `TimeToCloseFilter(min_hours=2, max_hours=504)`
- `AlreadyPositionedFilter`

### Scorer

```rust
async fn score(&self, context: &TradingContext, candidates: &[MarketCandidate]) -> Result<Vec<MarketCandidate>, String>
```

Scorers evaluate candidates and write scores into each candidate's `scores: HashMap<String, f64>` field. Scorers run in sequence. Each scorer receives the same full candidate slice and returns a scored copy. The `update_all` method merges the scores back into the live candidates by position.

A scorer writes one or more named keys, e.g., `"momentum_6h"`, `"volume_ratio"`. The final scorer in the default stack is `CategoryWeightedScorer`, which reads all accumulated scores and computes a single `final_score` on each candidate.

The `final_score` field is what the rest of the pipeline uses. If no scorer writes `final_score`, the selection and execution stages see zero scores everywhere.

### Selector

```rust
fn select(&self, context: &TradingContext, candidates: Vec<MarketCandidate>) -> Vec<MarketCandidate>
```

The selector is synchronous. It receives all scored candidates and returns a subset. The default selector is `TopKSelector`, which sorts by `final_score` descending and takes the top K.

The selector is where concentration limits are enforced. If you want to limit exposure to a single category, that logic belongs here, not in the filter stage.

### OrderExecutor

```rust
async fn execute_signal(&self, signal: &Signal, context: &TradingContext) -> Option<Fill>
fn generate_signals(&self, candidates: &[MarketCandidate], context: &TradingContext) -> Vec<Signal>
fn generate_exit_signals(&self, context: &TradingContext, candidate_scores: &HashMap<String, f64>) -> Vec<ExitSignal>
```

The executor has three responsibilities: generate entry signals from selected candidates, generate exit signals for current positions, and execute signals against the market (or a simulation).

In backtesting, `BacktestExecutor` simulates fills using historical prices with a configurable slippage. In paper trading, `PaperExecutor` calls the live API and applies Kelly-based position sizing.

The executor is also where fee filtering happens in live/paper mode: trades where the expected edge doesn't cover fees are dropped before submission.

## The TradingContext

Every stage receives a `TradingContext`:

```rust
pub struct TradingContext {
    pub timestamp: DateTime<Utc>,
    pub portfolio: Portfolio,
    pub trading_history: Vec<Trade>,
    pub request_id: String,
}
```

Context is read-only within a pipeline execution. Stages can read portfolio state to make decisions (e.g., `AlreadyPositionedFilter` checks `context.portfolio.has_position`), but they don't mutate it. The portfolio is updated only after execution, outside the pipeline.

## Two execution paths

### Backtest

The `Backtester` runs the pipeline in a time-stepped loop. At each step:

1. Resolve any markets that closed since the last step (apply market result to positions)
2. Run the pipeline to get selected candidates
3. Generate and execute exit signals for current positions
4. Generate and execute entry signals for selected candidates
5. Record equity snapshot

The loop advances by `interval_hours` until `end_time`.

### Paper trading

The `PaperTradingEngine` runs a similar loop, but on a real clock driven by `poll_interval_secs`. The source calls the live Kalshi API. The executor calls a simulated order book (`PaperExecutor`) — real prices, fake fills.

The web server (`pm-server`) runs alongside the engine and broadcasts WebSocket updates after each tick. Watchtower connects to this WebSocket to display live state.

## Observability points

Each stage writes to `PipelineResult`:

```rust
pub struct PipelineResult {
    pub retrieved_candidates: Vec<MarketCandidate>,   // after Source
    pub filtered_candidates: Vec<MarketCandidate>,    // after Filter
    pub selected_candidates: Vec<MarketCandidate>,    // after Selector
    pub context: Arc<TradingContext>,
}
```

Watchtower's pipeline metrics panel (`PipelineMetrics`) shows candidate counts at each stage per tick: fetched → filtered → selected → signals → fills. If you see a large drop between filtered and selected, the selector is being very aggressive. A large drop between fetched and filtered usually means the liquidity or time-to-close filter is removing most markets.
