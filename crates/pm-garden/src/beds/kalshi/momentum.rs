//! Momentum bed - specimens that chase price movement
//!
//! These specimens identify and trade with price trends.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pm_core::{MarketCandidate, Scorer, TradingContext};
use rust_decimal::prelude::ToPrimitive;

/// Simple price momentum scorer
pub struct MomentumScorer {
    lookback_hours: i64,
}

impl MomentumScorer {
    pub fn new(lookback_hours: i64) -> Self {
        Self { lookback_hours }
    }

    fn calculate_momentum(
        candidate: &MarketCandidate,
        now: DateTime<Utc>,
        lookback_hours: i64,
    ) -> f64 {
        let lookback_start = now - chrono::Duration::hours(lookback_hours);
        let relevant_history: Vec<_> = candidate
            .price_history
            .iter()
            .filter(|p| p.timestamp >= lookback_start)
            .collect();

        if relevant_history.len() < 2 {
            return 0.0;
        }

        let first = relevant_history
            .first()
            .unwrap()
            .yes_price
            .to_f64()
            .unwrap_or(0.5);
        let last = relevant_history
            .last()
            .unwrap()
            .yes_price
            .to_f64()
            .unwrap_or(0.5);

        last - first
    }
}

#[async_trait]
impl Scorer for MomentumScorer {
    fn name(&self) -> &'static str {
        "MomentumScorer"
    }

    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let momentum = Self::calculate_momentum(c, context.timestamp, self.lookback_hours);
                let mut scored = MarketCandidate {
                    scores: c.scores.clone(),
                    ..Default::default()
                };
                scored.scores.insert("momentum".to_string(), momentum);
                scored
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        if let Some(score) = scored.scores.get("momentum") {
            candidate.scores.insert("momentum".to_string(), *score);
        }
    }
}

/// Multi-timeframe momentum scorer
///
/// Looks at multiple windows and detects divergence between short and long term.
pub struct MultiTimeframeMomentumScorer {
    windows: Vec<i64>,
}

impl MultiTimeframeMomentumScorer {
    pub fn new(windows: Vec<i64>) -> Self {
        Self { windows }
    }

    pub fn default_windows() -> Self {
        Self::new(vec![1, 4, 12, 24])
    }

    fn calculate_momentum_for_window(
        candidate: &MarketCandidate,
        now: DateTime<Utc>,
        hours: i64,
    ) -> f64 {
        let lookback_start = now - chrono::Duration::hours(hours);
        let relevant_history: Vec<_> = candidate
            .price_history
            .iter()
            .filter(|p| p.timestamp >= lookback_start)
            .collect();

        if relevant_history.len() < 2 {
            return 0.0;
        }

        let first = relevant_history
            .first()
            .unwrap()
            .yes_price
            .to_f64()
            .unwrap_or(0.5);
        let last = relevant_history
            .last()
            .unwrap()
            .yes_price
            .to_f64()
            .unwrap_or(0.5);

        last - first
    }

    fn calculate_score(
        candidate: &MarketCandidate,
        now: DateTime<Utc>,
        windows: &[i64],
    ) -> (f64, f64, f64) {
        let momentums: Vec<f64> = windows
            .iter()
            .map(|&w| Self::calculate_momentum_for_window(candidate, now, w))
            .collect();

        if momentums.is_empty() {
            return (0.0, 0.0, 1.0);
        }

        let avg_momentum = momentums.iter().sum::<f64>() / momentums.len() as f64;

        let signs: Vec<i32> = momentums
            .iter()
            .map(|&m| {
                if m > 0.0 {
                    1
                } else if m < 0.0 {
                    -1
                } else {
                    0
                }
            })
            .collect();
        let all_same_sign = signs.iter().all(|&s| s == signs[0]) && signs[0] != 0;
        let alignment = if all_same_sign { 1.0 } else { 0.5 };

        let short_avg = if momentums.len() >= 2 {
            momentums[..momentums.len() / 2].iter().sum::<f64>() / (momentums.len() / 2) as f64
        } else {
            momentums[0]
        };
        let long_avg = if momentums.len() >= 2 {
            momentums[momentums.len() / 2..].iter().sum::<f64>()
                / (momentums.len() - momentums.len() / 2) as f64
        } else {
            momentums[0]
        };

        let divergence =
            if (short_avg > 0.0 && long_avg < 0.0) || (short_avg < 0.0 && long_avg > 0.0) {
                (short_avg - long_avg).abs()
            } else {
                0.0
            };

        (avg_momentum * alignment, divergence, alignment)
    }
}

#[async_trait]
impl Scorer for MultiTimeframeMomentumScorer {
    fn name(&self) -> &'static str {
        "MultiTimeframeMomentumScorer"
    }

    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let (momentum, divergence, alignment) =
                    Self::calculate_score(c, context.timestamp, &self.windows);
                let mut scored = MarketCandidate {
                    scores: c.scores.clone(),
                    ..Default::default()
                };
                scored.scores.insert("mtf_momentum".to_string(), momentum);
                scored
                    .scores
                    .insert("mtf_divergence".to_string(), divergence);
                scored.scores.insert("mtf_alignment".to_string(), alignment);
                scored
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        for key in ["mtf_momentum", "mtf_divergence", "mtf_alignment"] {
            if let Some(score) = scored.scores.get(key) {
                candidate.scores.insert(key.to_string(), *score);
            }
        }
    }
}

/// Time decay scorer - penalizes markets close to expiry
pub struct TimeDecayScorer;

impl TimeDecayScorer {
    pub fn new() -> Self {
        Self
    }

    fn calculate_time_decay(candidate: &MarketCandidate, now: DateTime<Utc>) -> f64 {
        let ttc = candidate.time_to_close(now);
        let hours_remaining = ttc.num_hours() as f64;

        if hours_remaining <= 0.0 {
            return -1.0;
        }

        let decay = 1.0 - (1.0 / (hours_remaining / 24.0 + 1.0));
        decay.min(1.0).max(0.0)
    }
}

impl Default for TimeDecayScorer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Scorer for TimeDecayScorer {
    fn name(&self) -> &'static str {
        "TimeDecayScorer"
    }

    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let time_decay = Self::calculate_time_decay(c, context.timestamp);
                let mut scored = MarketCandidate {
                    scores: c.scores.clone(),
                    ..Default::default()
                };
                scored.scores.insert("time_decay".to_string(), time_decay);
                scored
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        if let Some(score) = scored.scores.get("time_decay") {
            candidate.scores.insert("time_decay".to_string(), *score);
        }
    }
}
