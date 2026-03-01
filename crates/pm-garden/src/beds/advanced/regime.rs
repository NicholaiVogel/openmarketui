//! Regime detector identifying market states (bull, bear, neutral)

use async_trait::async_trait;
use pm_core::{MarketCandidate, Scorer, TradingContext};
use rust_decimal::prelude::ToPrimitive;

/// Regime detector identifying market states
pub struct RegimeDetector {
    lookback_hours: i64,
    trend_threshold: f64,
}

impl RegimeDetector {
    pub fn new(lookback_hours: i64) -> Self {
        Self {
            lookback_hours,
            trend_threshold: 0.05,
        }
    }

    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.trend_threshold = threshold;
        self
    }

    fn classify_regime(&self, prices: &[f64]) -> (f64, f64, f64) {
        if prices.len() < 10 {
            return (0.0, 0.0, 1.0);
        }

        let first = prices[0];
        let last = prices[prices.len() - 1];
        let trend = (last - first) / first;

        let returns: Vec<f64> = prices.windows(2).map(|w| (w[1] / w[0]).ln()).collect();

        let mean_return: f64 = returns.iter().sum::<f64>() / returns.len() as f64;
        let volatility: f64 = returns
            .iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>()
            / returns.len() as f64;

        let trend_score: f64 = if trend > self.trend_threshold {
            1.0
        } else if trend < -self.trend_threshold {
            -1.0
        } else {
            0.0
        };

        let regime_confidence = if trend_score.abs() > 0.0 { 0.8 } else { 0.4 };

        (trend_score, volatility, regime_confidence)
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

        let (trend_score, _volatility, confidence) = self.classify_regime(&prices);
        trend_score * confidence
    }
}

#[async_trait]
impl Scorer for RegimeDetector {
    fn name(&self) -> &'static str {
        "RegimeDetector"
    }

    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let regime_score = self.calculate_score(c, context.timestamp);
                let mut scored = MarketCandidate {
                    scores: c.scores.clone(),
                    ..Default::default()
                };
                scored.scores.insert("regime".to_string(), regime_score);
                scored
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        if let Some(score) = scored.scores.get("regime") {
            candidate.scores.insert("regime".to_string(), *score);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regime_detector_creation() {
        let detector = RegimeDetector::new(24);
        assert_eq!(detector.lookback_hours, 24);
        assert_eq!(detector.trend_threshold, 0.05);
    }

    #[test]
    fn test_classify_regime_insufficient_data() {
        let detector = RegimeDetector::new(24);
        let (trend, _vol, confidence) = detector.classify_regime(&[0.5, 0.5, 0.5]);
        assert_eq!(trend, 0.0);
        assert_eq!(confidence, 1.0);
    }
}
