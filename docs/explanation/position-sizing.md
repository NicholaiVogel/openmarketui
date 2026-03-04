# Position Sizing: Kelly Criterion and Risk Controls

This document explains how OpenMarketUI determines how large each position should be, and what guardrails prevent catastrophic losses.

## The goal

Position sizing answers one question: given that a scorer says this market looks good, how many contracts should we buy?

The naive answer (buy a fixed amount) ignores the edge size. Buying 100 contracts on a market with a 0.01 edge is the same bet size as a market with a 0.8 edge — which doesn't make sense. The Kelly criterion gives a principled answer: bet in proportion to your edge.

## Kelly criterion

The core formula (from `crates/pm-engine/src/execution.rs`):

```rust
let win_prob = edge_to_win_probability(edge);
let odds = (1.0 - price) / price;
let kelly = (odds * win_prob - (1.0 - win_prob)) / odds;
```

Breaking this down:

- `edge` is the `final_score` from the scorer — a continuous value representing directional confidence
- `edge_to_win_probability` maps this to [0, 1] via `(1 + tanh(edge)) / 2`. An edge of 0 → 50% win probability; a large positive edge approaches 100%
- `odds` is the payout ratio: how much you win relative to what you risk. A Yes contract at 0.30 pays 0.70 on a $0.30 stake, so odds = 0.70/0.30 ≈ 2.33
- The Kelly fraction is `(odds × win_prob - loss_prob) / odds`

The result is a fraction of bankroll. A full-Kelly bet of, say, 15% of bankroll is then scaled by `kelly_fraction` (the fractional Kelly multiplier) and capped by `max_position_pct`.

## Fractional Kelly

Full Kelly is theoretically optimal in the long run but causes extreme volatility and large drawdowns when your edge estimates are noisy — which they always are. The system uses **fractional Kelly**: the raw Kelly fraction is multiplied by `kelly_fraction` (default 0.40, meaning 40% of full Kelly).

```rust
let safe_kelly = (kelly * config.kelly_fraction).max(0.0);
let position_value = bankroll * safe_kelly.min(config.max_position_pct);
```

The `max_position_pct` cap adds a secondary safety net: even if the Kelly formula suggests a large bet, no single position can exceed this fraction of available cash. Default is 30% in backtesting, 10% in paper trading (more conservative because real capital is at risk).

## The four sizing parameters

`PositionSizingConfig` has four fields:

```rust
pub struct PositionSizingConfig {
    pub kelly_fraction: f64,      // multiplier on raw Kelly (0.0–1.0)
    pub max_position_pct: f64,    // hard cap as fraction of cash
    pub min_position_size: u64,   // minimum contracts (10 by default)
    pub max_position_size: u64,   // maximum contracts (100–2000 depending on mode)
}
```

**`kelly_fraction`**: How aggressive. 0.25 is conservative for live trading (25% of Kelly). 0.40 is the backtest default. Going above 0.5 approaches full Kelly and increases variance substantially.

**`max_position_pct`**: A second brake. Even if Kelly says 60% of bankroll, this caps it. In paper trading with $10,000, `max_position_pct = 0.10` means no position can exceed $1,000.

**`min_position_size`**: If Kelly computes a tiny bet (less than 10 contracts), we skip rather than bother. This avoids noise trades.

**`max_position_size`**: Hard contract ceiling. Independent of dollar value.

## Fee-aware filtering

Before generating a signal, the executor checks whether the expected edge actually covers fees:

```rust
let entry_fee_drag = fee_config.fee_drag_pct(quantity, price_f64);
let exit_fee_drag = fee_config.fee_drag_pct(quantity, 0.5);  // conservative estimate
let total_fee_drag = entry_fee_drag + exit_fee_drag;

if candidate.final_score.abs() < total_fee_drag + fee_config.min_edge_after_fees {
    return None;  // skip: insufficient edge after fees
}
```

Kalshi charges 7% of the profit on taker orders (the default). A trade with tiny edge and a 7% fee on exit will almost certainly lose money net of fees. `min_edge_after_fees = 0.02` means we require at least 2 points of edge beyond the fee drag to proceed.

## Preset configurations

`PositionSizingConfig` ships with three presets:

- `default()`: kelly_fraction=0.40, max_position_pct=0.30, max_position_size=1000
- `conservative()`: kelly_fraction=0.10, max_position_pct=0.10, max_position_size=500
- `aggressive()`: kelly_fraction=0.50, max_position_pct=0.40, max_position_size=2000

For live or paper trading, `conservative()` or something close to it is recommended until you have substantial backtest evidence that your scorers have real edge.

## Circuit breaker: portfolio-level risk

Kelly handles per-position sizing. The circuit breaker (`crates/pm-engine/src/circuit_breaker.rs`) handles portfolio-level risk:

```rust
pub struct CircuitBreakerConfig {
    pub max_drawdown_pct: f64,      // e.g., 0.15 → trip at 15% drawdown from peak
    pub max_daily_loss_pct: f64,    // e.g., 0.05 → trip at 5% daily loss
    pub max_positions: usize,       // hard cap on concurrent positions
    pub max_single_position_pct: f64,
    pub max_consecutive_errors: u32,
    pub max_fills_per_hour: u32,
    pub max_fills_per_day: u32,
}
```

When any rule is tripped, the engine stops entering new positions. Existing positions continue to be managed (exits still fire). The circuit breaker resets at the start of each session.

The `max_fills_per_hour` and `max_fills_per_day` limits are primarily to prevent runaway bugs — if the engine starts generating hundreds of fills unexpectedly, something is wrong and it should stop.

## The backtest vs. paper difference

In backtests, position sizing is simpler. The `BacktestExecutor.generate_signals` uses `max_position_size` as a fixed quantity — it doesn't call Kelly. This is intentional: backtests are for validating *which* candidates the scorers identify correctly, not for optimizing position sizes. Kelly sizing is applied to paper and live trading where capital efficiency matters.

If you want to test Kelly sizing in a backtest, use `BacktestExecutor.with_sizing_config()` to pass a `PositionSizingConfig` — the executor will use it if provided. This is how `Backtester::with_configs` works from the CLI.
