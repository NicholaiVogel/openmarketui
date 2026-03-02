//! Garden filters - the immune system
//!
//! Filters protect the garden from bad trades by removing
//! candidates that don't meet quality criteria.

use async_trait::async_trait;
use chrono::Duration;
use pm_core::{Filter, FilterResult, MarketCandidate, TradingContext};
use rust_decimal::prelude::ToPrimitive;
use std::collections::HashSet;

/// Liquidity filter - rejects illiquid specimens
pub struct LiquidityFilter {
    min_volume_24h: u64,
}

impl LiquidityFilter {
    pub fn new(min_volume_24h: u64) -> Self {
        Self { min_volume_24h }
    }
}

#[async_trait]
impl Filter for LiquidityFilter {
    fn name(&self) -> &'static str {
        "LiquidityFilter"
    }

    async fn filter(
        &self,
        _context: &TradingContext,
        candidates: Vec<MarketCandidate>,
    ) -> Result<FilterResult, String> {
        let (kept, removed): (Vec<_>, Vec<_>) = candidates
            .into_iter()
            .partition(|c| c.volume_24h >= self.min_volume_24h);

        Ok(FilterResult { kept, removed })
    }
}

/// Time to close filter - timing constraints
pub struct TimeToCloseFilter {
    min_hours: i64,
    max_hours: Option<i64>,
}

impl TimeToCloseFilter {
    pub fn new(min_hours: i64, max_hours: Option<i64>) -> Self {
        Self {
            min_hours,
            max_hours,
        }
    }
}

#[async_trait]
impl Filter for TimeToCloseFilter {
    fn name(&self) -> &'static str {
        "TimeToCloseFilter"
    }

    async fn filter(
        &self,
        context: &TradingContext,
        candidates: Vec<MarketCandidate>,
    ) -> Result<FilterResult, String> {
        let min_duration = Duration::hours(self.min_hours);
        let max_duration = self.max_hours.map(Duration::hours);

        let (kept, removed): (Vec<_>, Vec<_>) = candidates.into_iter().partition(|c| {
            let ttc = c.time_to_close(context.timestamp);
            let above_min = ttc >= min_duration;
            let below_max = max_duration.map(|max| ttc <= max).unwrap_or(true);
            above_min && below_max
        });

        Ok(FilterResult { kept, removed })
    }
}

/// Already positioned filter - prevents over-exposure
pub struct AlreadyPositionedFilter {
    max_position_per_market: u64,
}

impl AlreadyPositionedFilter {
    pub fn new(max_position_per_market: u64) -> Self {
        Self {
            max_position_per_market,
        }
    }
}

#[async_trait]
impl Filter for AlreadyPositionedFilter {
    fn name(&self) -> &'static str {
        "AlreadyPositionedFilter"
    }

    async fn filter(
        &self,
        context: &TradingContext,
        candidates: Vec<MarketCandidate>,
    ) -> Result<FilterResult, String> {
        let (kept, removed): (Vec<_>, Vec<_>) = candidates.into_iter().partition(|c| {
            context
                .portfolio
                .get_position(&c.ticker)
                .map(|p| p.quantity < self.max_position_per_market)
                .unwrap_or(true)
        });

        Ok(FilterResult { kept, removed })
    }
}

/// Category filter - whitelist or blacklist market categories
pub struct CategoryFilter {
    whitelist: Option<HashSet<String>>,
    blacklist: HashSet<String>,
}

impl CategoryFilter {
    pub fn whitelist(categories: Vec<String>) -> Self {
        Self {
            whitelist: Some(categories.into_iter().collect()),
            blacklist: HashSet::new(),
        }
    }

    pub fn blacklist(categories: Vec<String>) -> Self {
        Self {
            whitelist: None,
            blacklist: categories.into_iter().collect(),
        }
    }
}

#[async_trait]
impl Filter for CategoryFilter {
    fn name(&self) -> &'static str {
        "CategoryFilter"
    }

    async fn filter(
        &self,
        _context: &TradingContext,
        candidates: Vec<MarketCandidate>,
    ) -> Result<FilterResult, String> {
        let (kept, removed): (Vec<_>, Vec<_>) = candidates.into_iter().partition(|c| {
            let in_whitelist = self
                .whitelist
                .as_ref()
                .map(|w| w.contains(&c.category))
                .unwrap_or(true);
            let not_blacklisted = !self.blacklist.contains(&c.category);
            in_whitelist && not_blacklisted
        });

        Ok(FilterResult { kept, removed })
    }
}

/// Price range filter - only trade markets in certain price ranges
pub struct PriceRangeFilter {
    min_price: f64,
    max_price: f64,
}

impl PriceRangeFilter {
    pub fn new(min_price: f64, max_price: f64) -> Self {
        Self {
            min_price,
            max_price,
        }
    }

    /// Filter for markets with prices near 50% (high uncertainty)
    pub fn mid_range() -> Self {
        Self::new(0.35, 0.65)
    }

    /// Filter for extreme prices (likely to revert)
    pub fn extremes() -> Self {
        Self::new(0.0, 0.15)
    }
}

#[async_trait]
impl Filter for PriceRangeFilter {
    fn name(&self) -> &'static str {
        "PriceRangeFilter"
    }

    async fn filter(
        &self,
        _context: &TradingContext,
        candidates: Vec<MarketCandidate>,
    ) -> Result<FilterResult, String> {
        let (kept, removed): (Vec<_>, Vec<_>) = candidates.into_iter().partition(|c| {
            let price = c.current_yes_price.to_f64().unwrap_or(0.5);
            price >= self.min_price && price <= self.max_price
        });

        Ok(FilterResult { kept, removed })
    }
}

/// Volatility filter - only trade markets with certain volatility levels
pub struct VolatilityFilter {
    min_volatility: f64,
    max_volatility: f64,
}

impl VolatilityFilter {
    pub fn new(min_volatility: f64, max_volatility: f64) -> Self {
        Self {
            min_volatility,
            max_volatility,
        }
    }

    /// Filter for low volatility (stable) markets
    pub fn stable() -> Self {
        Self::new(0.0, 0.3)
    }

    /// Filter for high volatility (active) markets
    pub fn active() -> Self {
        Self::new(0.3, 1.0)
    }
}

#[async_trait]
impl Filter for VolatilityFilter {
    fn name(&self) -> &'static str {
        "VolatilityFilter"
    }

    async fn filter(
        &self,
        _context: &TradingContext,
        candidates: Vec<MarketCandidate>,
    ) -> Result<FilterResult, String> {
        let (kept, removed): (Vec<_>, Vec<_>) = candidates.into_iter().partition(|c| {
            let volatility = c.scores.get("volatility").copied().unwrap_or(0.0);
            volatility >= self.min_volatility && volatility <= self.max_volatility
        });

        Ok(FilterResult { kept, removed })
    }
}

/// Spread filter - rejects markets with wide bid-ask spreads
///
/// Uses the difference between yes and no prices as a proxy for spread
/// since MarketCandidate doesn't have explicit bid/ask.
pub struct SpreadFilter {
    max_spread: f64,
}

impl SpreadFilter {
    pub fn new(max_spread: f64) -> Self {
        Self { max_spread }
    }
}

#[async_trait]
impl Filter for SpreadFilter {
    fn name(&self) -> &'static str {
        "SpreadFilter"
    }

    async fn filter(
        &self,
        _context: &TradingContext,
        candidates: Vec<MarketCandidate>,
    ) -> Result<FilterResult, String> {
        let (kept, removed): (Vec<_>, Vec<_>) = candidates.into_iter().partition(|c| {
            // use yes + no prices as proxy (should sum to ~1.0 in efficient market)
            // wider spread = prices summing to > 1.0
            let yes = c.current_yes_price.to_f64().unwrap_or(0.5);
            let no = c.current_no_price.to_f64().unwrap_or(0.5);
            let implied_spread = (yes + no - 1.0).abs();
            implied_spread <= self.max_spread
        });

        Ok(FilterResult { kept, removed })
    }
}

/// Composite filter - combines multiple filters
pub struct CompositeFilter {
    filters: Vec<Box<dyn Filter>>,
}

impl CompositeFilter {
    pub fn new(filters: Vec<Box<dyn Filter>>) -> Self {
        Self { filters }
    }
}

#[async_trait]
impl Filter for CompositeFilter {
    fn name(&self) -> &'static str {
        "CompositeFilter"
    }

    async fn filter(
        &self,
        context: &TradingContext,
        mut candidates: Vec<MarketCandidate>,
    ) -> Result<FilterResult, String> {
        let mut all_removed = Vec::new();

        for filter in &self.filters {
            let result = filter.filter(context, candidates).await?;
            candidates = result.kept;
            all_removed.extend(result.removed);
        }

        Ok(FilterResult {
            kept: candidates,
            removed: all_removed,
        })
    }
}
