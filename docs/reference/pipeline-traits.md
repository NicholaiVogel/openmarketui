# Pipeline Traits API

All pipeline components are defined as Rust traits in `crates/pm-core/src/traits.rs`. Each trait has a default `enable()` method that returns `true` — components can override this to conditionally disable themselves based on context.

---

## Source

```rust
#[async_trait]
pub trait Source: Send + Sync {
    fn name(&self) -> &'static str;
    fn enable(&self, context: &TradingContext) -> bool { true }
    async fn get_candidates(&self, context: &TradingContext) -> Result<Vec<MarketCandidate>, String>;
}
```

Produces the initial candidate list for a pipeline run. Returns `Err(String)` on failure; the pipeline logs the error and proceeds with an empty candidate list.

**Implementations**:
- `HistoricalMarketSource` — queries `HistoricalData` for markets active at `context.timestamp`, providing a configurable lookback window of price history
- `LiveKalshiSource` — calls the Kalshi REST API; rate-limited to 2 requests/sec

---

## Filter

```rust
#[async_trait]
pub trait Filter: Send + Sync {
    fn name(&self) -> &'static str;
    fn enable(&self, context: &TradingContext) -> bool { true }
    async fn filter(
        &self,
        context: &TradingContext,
        candidates: Vec<MarketCandidate>,
    ) -> Result<FilterResult, String>;
}

pub struct FilterResult {
    pub kept: Vec<MarketCandidate>,
    pub removed: Vec<MarketCandidate>,
}
```

Takes the full candidate list and partitions it into `kept` and `removed`. Filters run sequentially — each filter receives only the candidates that survived all previous filters.

Return `Err` only on unexpected failure (e.g., a database lookup error). If a candidate simply doesn't pass the filter criteria, put it in `removed`, not `Err`.

The `removed` field exists for observability. Watchtower displays the count of removed candidates per tick. It has no effect on pipeline execution.

---

## Scorer

```rust
#[async_trait]
pub trait Scorer: Send + Sync {
    fn name(&self) -> &'static str;
    fn enable(&self, context: &TradingContext) -> bool { true }

    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String>;

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate);

    fn update_all(&self, candidates: &mut [MarketCandidate], scored: Vec<MarketCandidate>) {
        for (c, s) in candidates.iter_mut().zip(scored) {
            self.update(c, s);
        }
    }
}
```

Evaluates candidates and writes named scores into `candidate.scores: HashMap<String, f64>`.

**`score`** receives the full candidate slice as `&[MarketCandidate]` (not owned) and returns a scored copy. The pipeline calls `update_all` after to merge scores back.

**`update`** should copy only the keys this scorer is responsible for. Don't blindly replace the full `scores` HashMap — other scorers' keys must be preserved.

**Contract**:
- The returned `Vec` must have the same length and order as the input slice. The `update_all` default relies on positional alignment via `zip`.
- Score values have no required range, but by convention most scorers write values between -1.0 and +1.0. The ensemble scorer (CategoryWeightedScorer) treats positive scores as bullish (enter Yes-side if price < 0.5), negative as bearish.
- Write at least one key. A scorer that returns candidates unchanged has no effect.

---

## Selector

```rust
pub trait Selector: Send + Sync {
    fn name(&self) -> &'static str;
    fn enable(&self, context: &TradingContext) -> bool { true }
    fn select(
        &self,
        context: &TradingContext,
        candidates: Vec<MarketCandidate>,
    ) -> Vec<MarketCandidate>;
}
```

Synchronous. Takes all scored candidates and returns the subset to trade. Typically sorts by `final_score` and takes the top K.

**Implementations**:
- `TopKSelector` — sorts by `final_score` descending, takes the top K
- `ThresholdSelector` — keeps all candidates with `final_score.abs() > threshold`

---

## OrderExecutor

```rust
#[async_trait]
pub trait OrderExecutor: Send + Sync {
    async fn execute_signal(&self, signal: &Signal, context: &TradingContext) -> Option<Fill>;
    fn generate_signals(
        &self,
        candidates: &[MarketCandidate],
        context: &TradingContext,
    ) -> Vec<Signal>;
    fn generate_exit_signals(
        &self,
        context: &TradingContext,
        candidate_scores: &HashMap<String, f64>,
    ) -> Vec<ExitSignal>;
}
```

**`generate_signals`**: Converts selected candidates into entry `Signal` values. For each candidate, determines side (Yes/No based on score direction and price), computes position size (Kelly or fixed), and optionally applies fee filtering. Returns an empty vec if no candidates qualify.

**`generate_exit_signals`**: Examines current portfolio positions and returns `ExitSignal` values for positions that hit any exit rule (take profit, stop loss, time stop, score reversal). `candidate_scores` is a map from ticker to the latest `final_score` — used to detect score reversals.

**`execute_signal`**: Attempts to fill a signal. Returns `Some(Fill)` on success, `None` if the order can't be filled (insufficient cash, price limit exceeded, etc.). In backtesting, this looks up the historical price. In paper trading, this calls the `PaperExecutor`.

**Implementations**:
- `BacktestExecutor` — fills at historical price + slippage
- `PaperExecutor` — simulated fills at real-time prices, with Kelly sizing and fee deduction

---

## PipelineResult

```rust
pub struct PipelineResult {
    pub retrieved_candidates: Vec<MarketCandidate>,
    pub filtered_candidates: Vec<MarketCandidate>,
    pub selected_candidates: Vec<MarketCandidate>,
    pub context: Arc<TradingContext>,
}
```

Returned by `TradingPipeline::execute`. Contains candidate snapshots at three checkpoints in the pipeline. The `filtered_candidates` field contains the survivors after all filters have run (not per-filter). The `selected_candidates` field contains the output of the selector.

Note: `retrieved_candidates` is the pre-filter set (after Source). There is no post-scorer, pre-selector snapshot — by convention the scorer stage writes into `filtered_candidates` in place.
