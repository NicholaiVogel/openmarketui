# Tutorial: Building a Custom Scorer

In this tutorial you'll implement a new scorer, register it in the pipeline, and run a backtest that uses it. You'll understand the Scorer trait contract, how scores accumulate in a candidate, and how the ensemble combines them.

**Prerequisites**: Familiarity with Rust traits. Completed [Your First Backtest](01-first-backtest.md).

---

## What a scorer does

A scorer receives a slice of `MarketCandidate` values and returns a scored copy of each one. Scores are written into `candidate.scores: HashMap<String, f64>`. Multiple scorers run in sequence, each adding their own keys to the map. The final scorer (typically `CategoryWeightedScorer`) reads all accumulated keys and produces `candidate.final_score`.

The `final_score` is what the selector and executor use. If your scorer doesn't eventually feed into `final_score` (either directly or via the ensemble), it has no effect on trading decisions.

---

## Step 1: Create the scorer file

Create a new file in the appropriate bed. We'll write a simple "near-expiry value" scorer — it gives a positive signal to markets trading below 50 cents that close within 24 hours, on the theory that the market is pricing in uncertainty that will resolve shortly.

```bash
touch crates/pm-garden/src/beds/kalshi/near_expiry.rs
```

---

## Step 2: Implement the Scorer trait

```rust
// crates/pm-garden/src/beds/kalshi/near_expiry.rs

use pm_core::{MarketCandidate, Scorer, TradingContext};
use async_trait::async_trait;

/// Scores markets trading below 0.50 that expire within `window_hours`.
///
/// Rationale: markets with binary outcomes and short time to close have
/// most of their uncertainty resolved — late-breaking information tends
/// to move prices sharply. This scorer looks for potential mispricing.
pub struct NearExpiryValueScorer {
    /// Maximum hours to close to be considered "near expiry"
    pub window_hours: f64,
    /// Score weight written to the scores map
    pub weight: f64,
}

impl NearExpiryValueScorer {
    pub fn new(window_hours: f64) -> Self {
        Self {
            window_hours,
            weight: 1.0,
        }
    }
}

#[async_trait]
impl Scorer for NearExpiryValueScorer {
    fn name(&self) -> &'static str {
        "near_expiry_value"
    }

    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let now = context.timestamp;

        let scored = candidates.iter().map(|c| {
            let mut candidate = c.clone();
            let score = self.compute_score(c, now);
            candidate.scores.insert("near_expiry_value".to_string(), score);
            candidate
        }).collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        // merge only the keys this scorer writes
        if let Some(v) = scored.scores.get("near_expiry_value") {
            candidate.scores.insert("near_expiry_value".to_string(), *v);
        }
    }
}

impl NearExpiryValueScorer {
    fn compute_score(&self, candidate: &MarketCandidate, now: chrono::DateTime<chrono::Utc>) -> f64 {
        use rust_decimal::prelude::ToPrimitive;

        let Some(closes_at) = candidate.close_time else {
            return 0.0;
        };

        let hours_remaining = (closes_at - now).num_minutes() as f64 / 60.0;

        if hours_remaining <= 0.0 || hours_remaining > self.window_hours {
            return 0.0;
        }

        let yes_price = candidate.current_yes_price.to_f64().unwrap_or(0.5);

        // Signal is strongest for markets far from 50 cents (strong mispricing)
        // and closer to expiry (less time for the market to correct)
        let mispricing = (yes_price - 0.5).abs();
        let time_urgency = 1.0 - (hours_remaining / self.window_hours);

        // positive score if yes < 0.5 (favor Yes side), negative if yes > 0.5 (favor No)
        let direction = if yes_price < 0.5 { 1.0 } else { -1.0 };

        direction * mispricing * time_urgency * self.weight
    }
}
```

---

## Step 3: Export from the bed module

Add it to `crates/pm-garden/src/beds/kalshi/mod.rs`:

```rust
pub mod near_expiry;
pub use near_expiry::NearExpiryValueScorer;
```

And re-export from the garden root in `crates/pm-garden/src/lib.rs`:

```rust
pub use beds::kalshi::NearExpiryValueScorer;
```

---

## Step 4: Add to the default pipeline

The default backtest pipeline is built in `crates/pm-kalshi/src/backtest.rs` in `Backtester::build_default_pipeline`. Add your scorer to the scorers vec:

```rust
use pm_garden::NearExpiryValueScorer;

let scorers: Vec<Box<dyn Scorer>> = vec![
    Box::new(MomentumScorer::new(6)),
    Box::new(MultiTimeframeMomentumScorer::default_windows()),
    Box::new(MeanReversionScorer::new(24)),
    Box::new(BollingerMeanReversionScorer::default_config()),
    Box::new(VolumeScorer::new(6)),
    Box::new(OrderFlowScorer::new()),
    Box::new(TimeDecayScorer::new()),
    Box::new(NearExpiryValueScorer::new(24.0)),  // ← add this before the ensemble
    Box::new(CategoryWeightedScorer::with_defaults()),
];
```

Place it **before** `CategoryWeightedScorer` — the ensemble needs to see all scores when it computes `final_score`.

---

## Step 5: Register the score key in the ensemble

For `CategoryWeightedScorer` to include your scorer's output in `final_score`, you need to add the key to the weight map. Find `CategoryWeightedScorer::with_defaults()` in `crates/pm-garden/src/beds/kalshi/ensemble.rs` and add a weight:

```rust
weights.insert("near_expiry_value".to_string(), 0.15);
```

Adjust the weight relative to other scorers. Higher weight → more influence on `final_score`. Start conservatively (0.10–0.20) and adjust based on backtest results.

---

## Step 6: Run the backtest and compare

```bash
cargo build --release -p pm-kalshi

cargo run --release -p pm-kalshi -- run \
  --data-dir data \
  --start 2024-01-01 \
  --end 2024-06-01 \
  --capital 10000 \
  --compare-random
```

Compare the results with and without your scorer to measure its effect.

---

## What you've learned

- The `Scorer` trait requires `name()`, `score()`, and `update()`
- Scores accumulate in `candidate.scores: HashMap<String, f64>` across multiple scorers
- The ensemble scorer combines named scores into `final_score`
- Adding a scorer without registering its key in the ensemble has no effect on trading
- Build the default pipeline in `backtest.rs` to include your scorer for testing

**Next steps**:
- [Scorers and Filters Catalog](../reference/scorers-and-filters.md) — see how existing scorers are structured
- [Pipeline Traits API](../reference/pipeline-traits.md) — complete trait reference
- [Configure Exit Rules](../how-to/configure-exit-rules.md) — tune when your strategy exits positions
