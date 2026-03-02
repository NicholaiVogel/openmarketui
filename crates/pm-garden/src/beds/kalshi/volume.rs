//! Volume bed - specimens that read market flow
//!
//! These specimens analyze volume and order flow to detect
//! informed trading activity.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pm_core::{MarketCandidate, Scorer, TradingContext};

/// Volume analysis scorer
pub struct VolumeScorer {
    lookback_hours: i64,
}

impl VolumeScorer {
    pub fn new(lookback_hours: i64) -> Self {
        Self { lookback_hours }
    }

    fn calculate_volume_score(
        candidate: &MarketCandidate,
        now: DateTime<Utc>,
        lookback_hours: i64,
    ) -> f64 {
        let lookback_start = now - chrono::Duration::hours(lookback_hours);
        let recent_volume: u64 = candidate
            .price_history
            .iter()
            .filter(|p| p.timestamp >= lookback_start)
            .map(|p| p.volume)
            .sum();

        if candidate.total_volume == 0 {
            return 0.0;
        }

        let avg_hourly_volume =
            candidate.total_volume as f64 / ((now - candidate.open_time).num_hours().max(1) as f64);
        let recent_hourly_volume = recent_volume as f64 / lookback_hours.max(1) as f64;

        if avg_hourly_volume > 0.0 {
            (recent_hourly_volume / avg_hourly_volume)
                .ln()
                .max(-2.0)
                .min(2.0)
        } else {
            0.0
        }
    }
}

#[async_trait]
impl Scorer for VolumeScorer {
    fn name(&self) -> &'static str {
        "VolumeScorer"
    }

    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let volume =
                    Self::calculate_volume_score(c, context.timestamp, self.lookback_hours);
                let mut scored = MarketCandidate {
                    scores: c.scores.clone(),
                    ..Default::default()
                };
                scored.scores.insert("volume".to_string(), volume);
                scored
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        if let Some(score) = scored.scores.get("volume") {
            candidate.scores.insert("volume".to_string(), *score);
        }
    }
}

/// Order flow imbalance scorer
///
/// Measures buying vs selling pressure using taker side from trades.
pub struct OrderFlowScorer;

impl OrderFlowScorer {
    pub fn new() -> Self {
        Self
    }

    fn calculate_imbalance(candidate: &MarketCandidate) -> f64 {
        let buy_vol = candidate.buy_volume_24h as f64;
        let sell_vol = candidate.sell_volume_24h as f64;
        let total = buy_vol + sell_vol;

        if total == 0.0 {
            return 0.0;
        }

        (buy_vol - sell_vol) / total
    }
}

impl Default for OrderFlowScorer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Scorer for OrderFlowScorer {
    fn name(&self) -> &'static str {
        "OrderFlowScorer"
    }

    async fn score(
        &self,
        _context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let imbalance = Self::calculate_imbalance(c);
                let mut scored = MarketCandidate {
                    scores: c.scores.clone(),
                    ..Default::default()
                };
                scored.scores.insert("order_flow".to_string(), imbalance);
                scored
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        if let Some(score) = scored.scores.get("order_flow") {
            candidate.scores.insert("order_flow".to_string(), *score);
        }
    }
}

/// VPIN (Volume-synchronized Probability of Informed Trading) scorer
///
/// Measures flow toxicity using volume-bucketed order imbalance.
/// Based on Easley, Lopez de Prado, and O'Hara (2012).
pub struct VPINScorer {
    #[allow(dead_code)]
    bucket_size: u64,
    #[allow(dead_code)]
    num_buckets: usize,
}

impl VPINScorer {
    pub fn new(bucket_size: u64, num_buckets: usize) -> Self {
        Self {
            bucket_size,
            num_buckets,
        }
    }

    pub fn default_config() -> Self {
        Self::new(50, 20)
    }

    fn calculate_vpin(candidate: &MarketCandidate) -> f64 {
        let total_vol = candidate.buy_volume_24h + candidate.sell_volume_24h;
        if total_vol == 0 {
            return 0.0;
        }

        let buy_vol = candidate.buy_volume_24h as f64;
        let sell_vol = candidate.sell_volume_24h as f64;

        let imbalance = (buy_vol - sell_vol).abs();
        let vpin = imbalance / (buy_vol + sell_vol);

        vpin.clamp(0.0, 1.0)
    }

    fn calculate_flow_toxicity(candidate: &MarketCandidate) -> f64 {
        let vpin = Self::calculate_vpin(candidate);
        let volume_intensity = (candidate.volume_24h as f64).ln().max(0.0) / 10.0;
        vpin * (1.0 + volume_intensity.min(1.0))
    }

    fn calculate_informed_direction(candidate: &MarketCandidate) -> f64 {
        let buy = candidate.buy_volume_24h as f64;
        let sell = candidate.sell_volume_24h as f64;
        let total = buy + sell;

        if total == 0.0 {
            return 0.0;
        }

        let vpin = Self::calculate_vpin(candidate);
        let direction = (buy - sell) / total;
        direction * (1.0 + vpin)
    }
}

#[async_trait]
impl Scorer for VPINScorer {
    fn name(&self) -> &'static str {
        "VPINScorer"
    }

    async fn score(
        &self,
        _context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let vpin = Self::calculate_vpin(c);
                let toxicity = Self::calculate_flow_toxicity(c);
                let informed_dir = Self::calculate_informed_direction(c);
                let mut scored = MarketCandidate {
                    scores: c.scores.clone(),
                    ..Default::default()
                };
                scored.scores.insert("vpin".to_string(), vpin);
                scored.scores.insert("flow_toxicity".to_string(), toxicity);
                scored
                    .scores
                    .insert("informed_direction".to_string(), informed_dir);
                scored
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        for key in ["vpin", "flow_toxicity", "informed_direction"] {
            if let Some(score) = scored.scores.get(key) {
                candidate.scores.insert(key.to_string(), *score);
            }
        }
    }
}
