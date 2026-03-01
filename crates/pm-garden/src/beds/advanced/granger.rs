//! Granger causality based correlation scorer

use async_trait::async_trait;
use pm_core::{MarketCandidate, Scorer, TradingContext};
use rust_decimal::prelude::ToPrimitive;

/// Granger causality correlation scorer
pub struct GrangerCorrelationScorer {
    lookback_hours: i64,
    max_lag: usize,
}

impl GrangerCorrelationScorer {
    pub fn new(lookback_hours: i64, max_lag: usize) -> Self {
        Self {
            lookback_hours,
            max_lag,
        }
    }

    pub fn default_config() -> Self {
        Self::new(48, 5)
    }

    fn calculate_granger_causality(&self, series_x: &[f64], series_y: &[f64], lag: usize) -> f64 {
        if series_x.len() <= lag || series_y.len() <= lag {
            return 0.0;
        }

        let n = series_x.len().min(series_y.len()) - lag;
        if n < 10 {
            return 0.0;
        }

        let x_lagged: Vec<f64> = series_x[lag..].to_vec();
        let y_current: Vec<f64> = series_y[..n].to_vec();

        let x_pred: Vec<f64> = (0..n)
            .map(|i| {
                if i < lag {
                    series_x[i]
                } else {
                    series_x[i - lag]
                }
            })
            .collect();

        let mse_with_lag: f64 = x_lagged
            .iter()
            .zip(y_current.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f64>()
            / n as f64;

        let mse_baseline: f64 = x_pred
            .iter()
            .zip(y_current.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f64>()
            / n as f64;

        if mse_baseline > 0.0 {
            (mse_baseline - mse_with_lag) / mse_baseline
        } else {
            0.0
        }
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

        if prices.len() < self.max_lag * 2 + 10 {
            return 0.0;
        }

        let returns: Vec<f64> = prices.windows(2).map(|w| (w[1] / w[0]).ln()).collect();

        let mut max_causality: f64 = 0.0;
        for lag in 1..=self.max_lag {
            if lag >= returns.len() {
                break;
            }
            let causality = self.calculate_granger_causality(&returns, &returns, lag);
            max_causality = max_causality.max(causality);
        }

        max_causality.clamp(0.0, 1.0)
    }
}

#[async_trait]
impl Scorer for GrangerCorrelationScorer {
    fn name(&self) -> &'static str {
        "GrangerCorrelationScorer"
    }

    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let correlation_score = self.calculate_score(c, context.timestamp);
                let mut scored = MarketCandidate {
                    scores: c.scores.clone(),
                    ..Default::default()
                };
                scored
                    .scores
                    .insert("granger_correlation".to_string(), correlation_score);
                scored
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        if let Some(score) = scored.scores.get("granger_correlation") {
            candidate
                .scores
                .insert("granger_correlation".to_string(), *score);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let scorer = GrangerCorrelationScorer::default_config();
        assert_eq!(scorer.lookback_hours, 48);
        assert_eq!(scorer.max_lag, 5);
    }
}
