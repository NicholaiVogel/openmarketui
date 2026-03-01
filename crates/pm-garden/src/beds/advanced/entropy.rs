//! Entropy scorer measuring uncertainty in price distribution

use async_trait::async_trait;
use pm_core::{MarketCandidate, Scorer, TradingContext};
use rust_decimal::prelude::ToPrimitive;

/// Entropy scorer using Shannon entropy
pub struct EntropyScorer {
    lookback_hours: i64,
}

impl EntropyScorer {
    pub fn new(lookback_hours: i64) -> Self {
        Self { lookback_hours }
    }

    fn calculate_shannon_entropy(&self, prices: &[f64], bins: usize) -> f64 {
        if prices.is_empty() {
            return 0.0;
        }

        let min_price = prices.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_price = prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let bin_width = if max_price > min_price {
            (max_price - min_price) / bins as f64
        } else {
            0.01
        };

        let mut counts = vec![0.0; bins];
        for &price in prices {
            let bin_idx = if bin_width > 0.0 {
                ((price - min_price) / bin_width).floor() as usize
            } else {
                bins / 2
            };
            let idx = bin_idx.min(bins - 1);
            counts[idx] += 1.0;
        }

        let total = prices.len() as f64;
        let mut entropy = 0.0;
        for &count in &counts {
            if count > 0.0 {
                let p = count / total;
                entropy -= p * p.log2();
            }
        }

        entropy
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

        const ENTROPY_BINS: usize = 20;
        const MAX_ENTROPY: f64 = 4.0;
        let entropy = self.calculate_shannon_entropy(&prices, ENTROPY_BINS);
        // higher entropy = more uncertainty = lower score
        1.0 - (entropy / MAX_ENTROPY).clamp(0.0, 1.0)
    }
}

#[async_trait]
impl Scorer for EntropyScorer {
    fn name(&self) -> &'static str {
        "EntropyScorer"
    }

    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let entropy_score = self.calculate_score(c, context.timestamp);
                let mut scored = MarketCandidate {
                    scores: c.scores.clone(),
                    ..Default::default()
                };
                scored.scores.insert("entropy".to_string(), entropy_score);
                scored
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        if let Some(score) = scored.scores.get("entropy") {
            candidate.scores.insert("entropy".to_string(), *score);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entropy_scorer_creation() {
        let scorer = EntropyScorer::new(12);
        assert_eq!(scorer.lookback_hours, 12);
    }

    #[test]
    fn test_shannon_entropy_empty() {
        let scorer = EntropyScorer::new(12);
        assert_eq!(scorer.calculate_shannon_entropy(&[], 10), 0.0);
    }
}
