//! Volatility scorer with multiple estimators

use async_trait::async_trait;
use pm_core::{MarketCandidate, Scorer, TradingContext};
use rust_decimal::prelude::ToPrimitive;

/// Volatility scorer using realized volatility
pub struct VolatilityScorer {
    lookback_hours: i64,
}

impl VolatilityScorer {
    pub fn new(lookback_hours: i64) -> Self {
        Self { lookback_hours }
    }

    fn calculate_realized_volatility(&self, prices: &[f64]) -> f64 {
        if prices.len() < 2 {
            return 0.0;
        }

        let returns: Vec<f64> = prices.windows(2).map(|w| (w[1] / w[0]).ln()).collect();

        let mean: f64 = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance: f64 =
            returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;

        variance.sqrt()
    }

    fn calculate_score(
        &self,
        candidate: &MarketCandidate,
        now: chrono::DateTime<chrono::Utc>,
    ) -> f64 {
        let lookback_start = now - chrono::Duration::hours(self.lookback_hours);
        let prices: Vec<f64> = candidate
            .price_history
            .iter()
            .filter(|p| p.timestamp >= lookback_start)
            .filter_map(|p| p.yes_price.to_f64())
            .collect();

        if prices.len() < 10 {
            return 0.0;
        }

        const NORMALIZATION_FACTOR: f64 = 0.03;
        let realized_vol = self.calculate_realized_volatility(&prices);
        (realized_vol / NORMALIZATION_FACTOR).clamp(0.0, 1.0)
    }
}

#[async_trait]
impl Scorer for VolatilityScorer {
    fn name(&self) -> &'static str {
        "VolatilityScorer"
    }

    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let volatility = self.calculate_score(c, context.timestamp);
                let mut scored = MarketCandidate {
                    scores: c.scores.clone(),
                    ..Default::default()
                };
                scored.scores.insert("volatility".to_string(), volatility);
                scored
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        if let Some(score) = scored.scores.get("volatility") {
            candidate.scores.insert("volatility".to_string(), *score);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volatility_scorer_creation() {
        let scorer = VolatilityScorer::new(24);
        assert_eq!(scorer.lookback_hours, 24);
    }

    #[test]
    fn test_realized_volatility_empty() {
        let scorer = VolatilityScorer::new(24);
        assert_eq!(scorer.calculate_realized_volatility(&[]), 0.0);
    }
}
