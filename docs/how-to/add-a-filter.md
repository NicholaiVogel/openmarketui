# How to Add a Filter to the Immune System

Filters reject candidates before scoring begins. A good filter removes markets that are structurally unsuitable (too illiquid, closing at the wrong time, already positioned) without making any value judgments about market direction.

---

## Step 1: Implement the Filter trait

Create a file in `crates/pm-garden/src/filters/` or add to `mod.rs` for small filters:

```rust
// crates/pm-garden/src/filters/mod.rs (or a new file)

use pm_core::{Filter, FilterResult, MarketCandidate, TradingContext};
use async_trait::async_trait;

/// Filters out markets in specified categories
pub struct CategoryFilter {
    allowed: Option<Vec<String>>,
    blocked: Option<Vec<String>>,
}

impl CategoryFilter {
    /// Keep only markets in these categories
    pub fn new(allowed: Vec<String>) -> Self {
        Self { allowed: Some(allowed), blocked: None }
    }

    /// Remove markets in these categories
    pub fn excluded(blocked: Vec<String>) -> Self {
        Self { allowed: None, blocked: Some(blocked) }
    }
}

#[async_trait]
impl Filter for CategoryFilter {
    fn name(&self) -> &'static str {
        "category_filter"
    }

    async fn filter(
        &self,
        _context: &TradingContext,
        candidates: Vec<MarketCandidate>,
    ) -> Result<FilterResult, String> {
        let (kept, removed) = candidates.into_iter().partition(|c| {
            if let Some(ref allowed) = self.allowed {
                return allowed.contains(&c.category);
            }
            if let Some(ref blocked) = self.blocked {
                return !blocked.contains(&c.category);
            }
            true
        });

        Ok(FilterResult { kept, removed })
    }
}
```

**Key points**:
- `filter` takes ownership of `candidates` and returns a `FilterResult` with `kept` and `removed`
- Both fields in `FilterResult` must be populated — don't leave `removed` empty if you reject any candidates
- Return `Err` only for unexpected failures (API calls, database errors), not for normal filtering logic

---

## Step 2: Export the filter

In `crates/pm-garden/src/filters/mod.rs`, ensure it's exported:

```rust
pub use self::CategoryFilter;
```

And from the garden root in `crates/pm-garden/src/lib.rs`:

```rust
pub use filters::CategoryFilter;
```

---

## Step 3: Add to the pipeline

**In backtesting** (`crates/pm-kalshi/src/backtest.rs`):

```rust
use pm_garden::CategoryFilter;

let filters: Vec<Box<dyn Filter>> = vec![
    Box::new(LiquidityFilter::new(10)),
    Box::new(TimeToCloseFilter::new(0, None)),
    Box::new(AlreadyPositionedFilter::new(config.max_position_size)),
    Box::new(CategoryFilter::excluded(vec!["Sports".to_string()])),  // ← add here
];
```

**In paper trading** (`crates/pm-kalshi/src/main.rs` or wherever the paper pipeline is built), add to the equivalent filter vec.

Filters run in the order they appear in the vec. Place computationally cheap filters first (category checks, already-positioned checks) and more expensive ones last.

---

## Step 4: Verify with the pipeline funnel

Run a backtest and watch the pipeline funnel output:

```
INFO  data loaded candidates=847
INFO  filtered candidates=312 removed=535
```

If your new filter is removing more candidates than expected, add a log statement inside your filter to see which candidates are being rejected and why:

```rust
async fn filter(&self, ...) -> Result<FilterResult, String> {
    let (kept, removed): (Vec<_>, Vec<_>) = candidates.into_iter().partition(|c| { ... });

    if !removed.is_empty() {
        tracing::debug!(
            filter = self.name(),
            removed = removed.len(),
            kept = kept.len(),
            "filter applied"
        );
    }

    Ok(FilterResult { kept, removed })
}
```

Run with `RUST_LOG=debug` to see the output.

---

## When to use a filter vs. a scorer

Use a **filter** when a market is categorically unsuitable:
- Too illiquid to trade without moving the market
- Closing at the wrong time
- Already at maximum position
- In a category you've explicitly excluded

Use a **scorer** when a market might be worth trading depending on other factors:
- Price momentum suggests a direction
- Volume is elevated but not prohibitive
- The market is near a resistance level

If you find yourself writing a filter that returns a partial score or needs to look at price history, it's probably a scorer. Filters should be fast, stateless, and deterministic.
