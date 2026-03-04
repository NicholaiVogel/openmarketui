# Scorers and Filters Catalog

All scorers and filters live in `crates/pm-garden/`. Scorers are organized into "beds" by strategy family. Filters are the "immune system" and live in `src/filters/`.

---

## Filters (Immune System)

Filters are in `crates/pm-garden/src/filters/mod.rs`. They make binary keep/remove decisions — they don't rank candidates.

### LiquidityFilter

```rust
LiquidityFilter::new(min_volume: u64)
```

Removes candidates with `volume_24h < min_volume`. Use to exclude thinly traded markets where fills would move the price or be difficult to exit.

- Backtest default: `min_volume = 10` (permissive, historical data is sparse)
- Paper trading default: `min_volume = 100`

### TimeToCloseFilter

```rust
TimeToCloseFilter::new(min_hours: i64, max_hours: Option<i64>)
```

Removes candidates that close too soon or too far in the future. Requires `candidate.close_time` to be set. Markets with no close time pass through.

- `min_hours`: Reject markets closing in fewer than this many hours
- `max_hours`: Reject markets not closing within this many hours (None = no upper bound)

Paper trading defaults: `min_hours = 2`, `max_hours = Some(504)` (21 days)

### AlreadyPositionedFilter

```rust
AlreadyPositionedFilter::new(max_position_size: u64)
```

Removes candidates where `context.portfolio.has_position(ticker)` returns true AND current quantity ≥ `max_position_size`. Prevents opening duplicate positions beyond the size limit.

### SpreadFilter

Removes candidates with a bid-ask spread above a configurable threshold. Useful for filtering out markets where the cost of entering and exiting is too high.

### PriceRangeFilter

```rust
PriceRangeFilter::new(min_price: Decimal, max_price: Decimal)
```

Removes candidates whose Yes price falls outside the specified range. Useful for avoiding extreme contracts (e.g., Yes < 0.02 or Yes > 0.98) where edge is hard to find.

### CategoryFilter

```rust
CategoryFilter::new(allowed: Vec<String>)
// or
CategoryFilter::excluded(blocked: Vec<String>)
```

Include only candidates in specified categories, or exclude specified categories. Category values come from the Kalshi API (e.g., `"Politics"`, `"Economics"`, `"Sports"`).

### VolatilityFilter

Removes candidates with recent price volatility outside acceptable bounds. Useful for filtering markets that are moving too fast (unreliable signals) or not moving at all (no edge opportunity).

### CompositeFilter

```rust
CompositeFilter::new(filters: Vec<Box<dyn Filter>>)
```

Chains multiple filters sequentially. Equivalent to adding multiple filters to the pipeline directly, but allows building reusable filter sets.

---

## Kalshi Bed Scorers

The Kalshi bed (`crates/pm-garden/src/beds/kalshi/`) contains production scorers for Kalshi markets.

### MomentumScorer

```rust
MomentumScorer::new(window: usize)
```

Computes the percentage price change over the last `window` price history points. Positive score = Yes price is trending up (favor Yes). Negative = trending down (favor No).

Score key: `"momentum_{window}h"` (e.g., `"momentum_6h"`)

### MultiTimeframeMomentumScorer

```rust
MultiTimeframeMomentumScorer::default_windows()
// default windows: [3, 6, 12, 24]
```

Runs `MomentumScorer` at multiple time windows and combines them. Earlier windows get higher weight. Captures both short-term and medium-term trends simultaneously.

Score key: `"momentum_mtf"`

### TimeDecayScorer

```rust
TimeDecayScorer::new()
```

Scores based on time remaining to close. Markets with less time to close get higher scores (in absolute terms), reflecting reduced uncertainty. Works as a multiplier on other signals — a market closing in 2 hours with a strong momentum signal is more actionable than the same signal on a 30-day market.

Score key: `"time_decay"`

### MeanReversionScorer

```rust
MeanReversionScorer::new(lookback: usize)
```

Computes how far the current price deviates from its simple moving average over `lookback` periods. Positive deviation (price above SMA) → bearish signal (favor No). Negative deviation → bullish (favor Yes).

Score key: `"mean_reversion"`

### BollingerMeanReversionScorer

```rust
BollingerMeanReversionScorer::default_config()
```

Uses Bollinger Bands (typically ±2 standard deviations from the SMA). When price touches the lower band, generates a bullish signal. Upper band → bearish. More sophisticated than `MeanReversionScorer` because it normalizes by volatility.

Score key: `"bollinger_reversion"`

### VolumeScorer

```rust
VolumeScorer::new(window: usize)
```

Compares recent volume to the historical average. High recent volume relative to baseline suggests information is arriving (market activity is meaningful). Low relative volume suggests a dormant market.

Score key: `"volume_ratio"`

### VPINScorer

Volume-Price Imbalance Notional. Estimates the flow toxicity — the proportion of volume driven by informed traders versus noise traders. High VPIN suggests informed order flow, which is predictive of price movement.

Score key: `"vpin"`

### OrderFlowScorer

```rust
OrderFlowScorer::new()
```

Analyzes the imbalance between buyer-initiated and seller-initiated volume in recent price history. Persistent buying pressure → bullish signal. Persistent selling pressure → bearish.

Score key: `"order_flow"`

### CategoryWeightedScorer

```rust
CategoryWeightedScorer::with_defaults()
```

**This is the ensemble scorer.** It reads all accumulated score keys and computes `candidate.final_score` as a weighted sum. Must run last in the scorer chain.

Default weights (approximate):
- `"momentum_6h"`: moderate weight
- `"momentum_mtf"`: moderate weight
- `"volume_ratio"`: lower weight
- `"order_flow"`: lower weight
- `"time_decay"`: multiplier (amplifies other signals for near-expiry markets)
- `"bollinger_reversion"`: moderate weight

Weights are category-specific — the ensemble can apply different weights for `"Politics"` vs `"Economics"` markets based on which signals tend to work in each category.

When adding a new scorer, register its key here with an initial weight of 0.10–0.15. The weight is a starting point; tune based on attribution analysis.

Score key: writes `"final_score"` directly to `candidate.final_score`

### WeightedScorer

```rust
WeightedScorer::new(weights: HashMap<String, f64>)
```

Simpler version of `CategoryWeightedScorer` with uniform weights across all categories. Useful for prototyping before adding category-specific logic.

### BayesianEnsembleScorer

Combines scorer outputs using Bayesian updating rather than linear weighting. Treats each scorer as an independent signal updating a prior probability. More robust to correlated scorers than linear combination.

---

## Advanced Bed Scorers

The advanced bed (`crates/pm-garden/src/beds/advanced/`) contains experimental and research-grade scorers. These are more computationally expensive and may require longer price history.

### EntropyScorer

Measures the entropy (information content) of recent price movements. High entropy = chaotic, unpredictable price action. Low entropy = structured movement (trend or mean reversion). Used to gate other signals — don't rely on momentum signals when entropy is high.

### GrangerCorrelationScorer

Tests for Granger causality between correlated markets. If market A's price history predicts market B's price movement (not just correlation, but predictive causality), generates a lead-lag signal. Requires a reference market or set of correlated markets.

### KalmanPriceFilter

Applies a Kalman filter to the price history to separate signal from noise. Produces a smoothed price estimate and a velocity estimate (trend direction). More robust to outlier prices than simple moving averages.

### MomentumAccelerationScorer

Measures the second derivative of price momentum — whether momentum is speeding up or slowing down. Positive acceleration on an uptrend → strengthen bullish signal. Decelerating momentum → early warning of reversal.

### RegimeDetector / RegimeAdaptiveScorer

Attempts to classify the current market regime (trending, mean-reverting, high-volatility) and adapts which signals to weight more heavily. In trending regimes, momentum signals are reliable. In mean-reverting regimes, Bollinger/mean-reversion signals are better.

### VolatilityScorer

Measures recent price volatility. Can be used as a signal (high volatility preceding binary resolution is predictive) or as a gating condition for other scorers.

---

## OSINT Scorer

```
crates/pm-garden/src/beds/kalshi/osint.rs
```

Open-source intelligence scorer. Integrates external signal sources (news, social data) to generate scores based on external information rather than just market price history. The specific data sources are configured at construction time.

---

## Adding a scorer or filter

See [Building a Custom Scorer](../tutorials/02-building-a-custom-scorer.md) for a step-by-step walkthrough. The key points:

1. Implement `Scorer` or `Filter` from `pm_core`
2. Write to unique score keys (for scorers)
3. Register keys in `CategoryWeightedScorer::with_defaults()` (for scorers that should affect `final_score`)
4. Export from the bed's `mod.rs` and from `pm-garden/src/lib.rs`
5. Add to the pipeline in `backtest.rs` or the paper trading startup
