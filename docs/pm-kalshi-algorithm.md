pm-kalshi algorithm (how it works)
===

This documents the current behavior of the `crates/pm-kalshi/` Kalshi engine.
It is code-referential on purpose: each step links to the module that
implements it.


mental model
---

`pm-kalshi` is a pipeline-driven trader:

Source -> Filter -> Scorer -> Selector -> OrderExecutor

- Sources create `MarketCandidate` objects.
- Filters remove candidates that should not be traded.
- Scorers write signal features into `candidate.scores` and/or compute a
  combined `candidate.final_score`.
- The selector chooses which candidates to consider for entry.
- The executor turns scored candidates into entry signals, applies sizing,
  and executes fills; it also computes exit signals for existing positions.

Core types and trait interfaces live in:
- `crates/pm-core/src/types.rs`
- `crates/pm-core/src/traits.rs`


two operating paths
---

Backtest:
- Orchestrator: `crates/pm-kalshi/src/backtest.rs` (`Backtester`)
- Candidates from historical CSVs: `crates/pm-kalshi/src/data/loader.rs` and
  `crates/pm-kalshi/src/sources/historical.rs` (`HistoricalMarketSource`)
- Entry sizing: currently a simple fixed-size generator via
  `crates/pm-engine/src/execution.rs` (`simple_signal_generator`)

Paper trading loop:
- Orchestrator: `crates/pm-kalshi/src/engine/trading.rs` (`PaperTradingEngine::tick`)
- Candidates from live API: `crates/pm-kalshi/src/sources/live.rs`
  (`LiveKalshiSource`)
- Entry sizing: Kelly-based per-candidate sizing via
  `crates/pm-engine/src/execution.rs` (`candidate_to_signal`, `kelly_size`)
- Risk guardrails: `crates/pm-engine/src/circuit_breaker.rs`
- Persistence: `pm_store` (`record_fill`, `save_portfolio`, equity snapshots)


the pipeline (shared core)
---

Pipeline implementation:
- `crates/pm-kalshi/src/pipeline/trading_pipeline.rs` (`TradingPipeline::execute`)

Execution order in one pipeline run:
1. Fetch candidates from each enabled source.
2. Apply each enabled filter, in order.
   - If a filter errors, the pipeline logs it and restores the previous
     candidate list (best-effort robustness).
3. Run each enabled scorer, in order.
   - Scorers return a scored copy list; then `update_all` merges results into
     the original candidates.
4. Select candidates via the configured selector.
   - `TopKSelector` sorts by `final_score` and truncates.
   - `ThresholdSelector` filters by `final_score >= min_score`, then sorts.
   - Selectors live in `crates/pm-kalshi/src/pipeline/selector.rs`.


default filters
---

Backtest default pipeline builder:
- `crates/pm-kalshi/src/backtest.rs` (`Backtester::build_default_pipeline`)

Filters (in order):
- `LiquidityFilter::new(100)` (drops illiquid markets)
- `TimeToCloseFilter::new(2, Some(720))` (keeps markets 2h..30d to close)
- `AlreadyPositionedFilter::new(max_position_size)` (avoid over-allocating to
  a ticker)

Paper engine pipeline builder:
- `crates/pm-kalshi/src/engine/trading.rs` (`PaperTradingEngine::build_pipeline`)

Filters (in order):
- `TimeToCloseFilter::new(min_hours, Some(max_hours))` (from config)
- `AlreadyPositionedFilter::new(max_pos_size.max(100))` where:
  `max_pos_size = (initial_capital * max_position_pct) as u64`


default scorers (feature generation)
---

These scorers populate `candidate.scores` (feature keys shown in backticks).
Most are implemented in `pm-garden` and re-exported by
`crates/pm-kalshi/src/pipeline/mod.rs`.

Momentum bed:
- `MomentumScorer(lookback_hours)` (`momentum`)
  - `crates/pm-garden/src/beds/kalshi/momentum.rs`
- `MultiTimeframeMomentumScorer::default_windows()`
  (`mtf_momentum`, `mtf_divergence`, `mtf_alignment`)
  - `crates/pm-garden/src/beds/kalshi/momentum.rs`

Mean reversion bed:
- `MeanReversionScorer(lookback_hours)` (`mean_reversion`)
  - `crates/pm-garden/src/beds/kalshi/mean_reversion.rs`
- `BollingerMeanReversionScorer::default_config()`
  (`bollinger_reversion`, `bollinger_position`)
  - `crates/pm-garden/src/beds/kalshi/mean_reversion.rs`

Volume / flow bed:
- `VolumeScorer(lookback_hours)` (`volume`)
  - `crates/pm-garden/src/beds/kalshi/volume.rs`
- `OrderFlowScorer` (`order_flow`)
  - `crates/pm-garden/src/beds/kalshi/volume.rs`

Time preference:
- `TimeDecayScorer` (`time_decay`)
  - `crates/pm-garden/src/beds/kalshi/momentum.rs`


final_score (the ensemble step)
---

Selection and execution use `candidate.final_score`, not individual keys.

In the current default pipelines, `final_score` is computed by:
- `CategoryWeightedScorer::with_defaults()`
  - `crates/pm-garden/src/beds/kalshi/ensemble.rs`

Behavior:
- It picks a weight vector based on `candidate.category` (lowercased), with a
  fallback default.
- It computes a weighted sum over the feature keys:
  - `momentum`, `mean_reversion`, `volume`, `time_decay`, `order_flow`,
    `bollinger_reversion`, `mtf_momentum`
- It writes the result into `candidate.final_score`.

Important: any scorer that runs after `CategoryWeightedScorer` can overwrite
`final_score` again, depending on how it updates the candidate.


selection
---

The default selector is:
- `TopKSelector::new(max_positions)`
  - `crates/pm-kalshi/src/pipeline/selector.rs`

This sorts descending by `final_score` and truncates to K.


entry signals and sizing
---

Paper trading entries:
- Entry signals are generated in `crates/pm-kalshi/src/sources/paper_executor.rs`
  via `pm_engine::candidate_to_signal(...)`.
- That function:
  - chooses a side based on `final_score` sign and whether YES is above/below
    0.5 ("buy the cheaper side" heuristic)
  - computes Kelly-based quantity via `pm_engine::kelly_size(...)`
  - enforces `min_position_size`, `max_position_size`, and affordability
  - emits `Signal { limit_price: Some(current_price) }`
  - implementation: `crates/pm-engine/src/execution.rs`

Backtest entries:
- Backtest currently uses `pm_engine::simple_signal_generator(...)` which:
  - only enters candidates with `final_score > 0.0`
  - uses a fixed `quantity = position_size` (no Kelly)
  - buys YES if YES < 0.5 else buys NO
  - implementation: `crates/pm-engine/src/execution.rs`

Net effect: backtest and paper do not currently trade the same entry policy.


exit signals
---

Exit computation is shared:
- `pm_engine::compute_exit_signals(...)` in `crates/pm-engine/src/execution.rs`

For each open position:
1. Time stop (first): if held >= `max_hold_hours`, emit an exit.
2. Take profit: if pnl_pct >= `take_profit_pct`, emit an exit.
3. Stop loss: if pnl_pct <= -`stop_loss_pct`, emit an exit.
4. Score reversal: if `candidate_scores[ticker] < score_reversal_threshold`,
   emit an exit.

Notes:
- For non-time exits, a current price is required.
- PnL uses an "effective price" that flips NO positions as `1 - yes_price`.


paper engine tick loop (one cycle)
---

Implementation: `crates/pm-kalshi/src/engine/trading.rs` (`PaperTradingEngine::tick`)

On each tick:
1. Update context timestamp + request id.
2. Run the pipeline -> get `retrieved_candidates` and `selected_candidates`.
3. Update the executor's price cache from `retrieved_candidates`.
4. Compute exit signals for current positions and execute exits.
5. Generate entry signals from `selected_candidates` and execute entries,
   applying throttles:
   - max positions
   - max entries per tick
   - cash reserve floor
6. Run circuit breaker checks (drawdown, daily loss, fill rate, etc.) and
   pause engine if tripped.
7. Persist portfolio + equity snapshot + pipeline run metrics.


metrics and reporting
---

Backtest metrics:
- Collected in `crates/pm-kalshi/src/metrics.rs` (`MetricsCollector`)
- Stored in `BacktestResult` and printed via `BacktestResult::summary()`

Paper tick metrics:
- `TickMetrics` broadcast from `crates/pm-kalshi/src/engine/trading.rs` for the
  web UI.


known sharp edges (current behavior)
---

1) Live candidates have no history/flow
- `LiveKalshiSource` currently sets `price_history = Vec::new()` and
  `buy_volume_24h = 0`, `sell_volume_24h = 0`.
- This means momentum/mean-reversion/volume/order-flow scorers will generally
  output 0.0 for live trading.

2) TimeDecay-only trading is possible in paper mode
- Even with empty history, `TimeDecayScorer` produces a positive `time_decay`.
- `CategoryWeightedScorer` then turns that into a positive `final_score`, which
  can cause the engine to enter positions primarily based on time-to-close.

3) Backtest vs paper entry policy mismatch
- Backtest uses `simple_signal_generator` (fixed size, only `final_score > 0`).
- Paper uses `candidate_to_signal` (Kelly sizing, sign-aware exits, per-ticker
  max sizing).


where to change behavior (common edits)
---

- Add live price history / flow features:
  `crates/pm-kalshi/src/sources/live.rs`
- Change which filters/scorers are active in paper mode:
  `crates/pm-kalshi/src/engine/trading.rs` (`build_pipeline`)
- Make backtest and paper share the same entry logic:
  `crates/pm-kalshi/src/backtest.rs` (`BacktestExecutor::generate_signals`)
  and `crates/pm-kalshi/src/sources/paper_executor.rs`
- Adjust exit rules:
  `crates/pm-engine/src/execution.rs` (`compute_exit_signals`) and
  config defaults in `crates/pm-kalshi/src/config/mod.rs`
- Adjust category weights / what features contribute to `final_score`:
  `crates/pm-garden/src/beds/kalshi/ensemble.rs`


running it
---

CLI entrypoint:
- `crates/pm-kalshi/src/main.rs`

Common commands:
- Backtest: `cargo run -p pm-kalshi -- run --data-dir data --start YYYY-MM-DD --end YYYY-MM-DD ...`
- Paper: `cargo run -p pm-kalshi -- paper --config config.toml`
