//! Momentum acceleration scorer (second-order momentum)
//!
//! Detects changes in the rate of price movement

use async_trait::async_trait;
use pm_core::{MarketCandidate, Scorer, TradingContext};
use rust_decimal::prelude::ToPrimitive;

/// Momentum acceleration scorer detecting changes in momentum rate
pub struct MomentumAccelerationScorer {
    fast_window: i64,
    slow_window: i64,
}

impl MomentumAccelerationScorer {
    const REGIME_BULL: f64 = 1.0;
    const REGIME_BEAR: f64 = -1.0;
    const REGIME_CORRECTION: f64 = -0.5;
    const REGIME_RECOVERY: f64 = 0.5;

    pub fn new(fast_window: i64, slow_window: i64) -> Self {
        Self {
            fast_window,
            slow_window,
        }
    }

    pub fn default_config() -> Self {
        Self::new(3, 12)
    }

    fn calculate_momentum(
        candidate: &MarketCandidate,
        now: chrono::DateTime<chrono::Utc>,
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

    fn calculate_acceleration(
        candidate: &MarketCandidate,
        now: chrono::DateTime<chrono::Utc>,
        fast_window: i64,
        slow_window: i64,
    ) -> (f64, f64, f64) {
        let fast_mom = Self::calculate_momentum(candidate, now, fast_window);
        let slow_mom = Self::calculate_momentum(candidate, now, slow_window);

        let acceleration = if slow_mom.abs() > 0.001 {
            (fast_mom - slow_mom) / slow_mom.abs()
        } else {
            fast_mom * 10.0
        };

        let regime: f64 = if fast_mom > 0.0 && slow_mom > 0.0 {
            Self::REGIME_BULL
        } else if fast_mom < 0.0 && slow_mom < 0.0 {
            Self::REGIME_BEAR
        } else if slow_mom > 0.0 && fast_mom < 0.0 {
            Self::REGIME_CORRECTION
        } else {
            Self::REGIME_RECOVERY
        };

        let turning_point = if acceleration.abs() > 0.5 && regime.abs() < 1.0 {
            acceleration.signum() * 0.5
        } else {
            0.0
        };

        (acceleration.clamp(-2.0, 2.0), regime, turning_point)
    }
}

#[async_trait]
impl Scorer for MomentumAccelerationScorer {
    fn name(&self) -> &'static str {
        "MomentumAccelerationScorer"
    }

    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let (acceleration, regime, turning_point) = Self::calculate_acceleration(
                    c,
                    context.timestamp,
                    self.fast_window,
                    self.slow_window,
                );
                let mut scored = MarketCandidate {
                    scores: c.scores.clone(),
                    ..Default::default()
                };
                scored
                    .scores
                    .insert("momentum_acceleration".to_string(), acceleration);
                scored.scores.insert("momentum_regime".to_string(), regime);
                scored
                    .scores
                    .insert("turning_point".to_string(), turning_point);
                scored
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        for key in ["momentum_acceleration", "momentum_regime", "turning_point"] {
            if let Some(score) = scored.scores.get(key) {
                candidate.scores.insert(key.to_string(), *score);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let scorer = MomentumAccelerationScorer::default_config();
        assert_eq!(scorer.fast_window, 3);
        assert_eq!(scorer.slow_window, 12);
    }
}
