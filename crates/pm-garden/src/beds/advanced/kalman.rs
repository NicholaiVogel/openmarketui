//! Kalman filter for price estimation

use async_trait::async_trait;
use pm_core::{MarketCandidate, Scorer, TradingContext};
use rust_decimal::prelude::ToPrimitive;

/// Kalman filter for adaptive price estimation
pub struct KalmanPriceFilter {
    lookback_hours: i64,
    process_noise: f64,
    measurement_noise: f64,
}

impl KalmanPriceFilter {
    pub fn new(lookback_hours: i64, process_noise: f64, measurement_noise: f64) -> Self {
        Self {
            lookback_hours,
            process_noise,
            measurement_noise,
        }
    }

    pub fn default_config() -> Self {
        Self::new(24, 0.001, 0.01)
    }

    fn run_kalman_filter(&self, prices: &[f64]) -> (f64, f64) {
        if prices.is_empty() {
            return (0.5, 1.0);
        }

        let mut x = prices[0];
        let mut p = 1.0;

        let q = self.process_noise;
        let r = self.measurement_noise;

        for &z in prices.iter().skip(1) {
            let x_pred = x;
            let p_pred = p + q;

            let k = p_pred / (p_pred + r);
            x = x_pred + k * (z - x_pred);
            p = (1.0 - k) * p_pred;
        }

        (x, p.sqrt())
    }

    fn calculate_score(
        &self,
        candidate: &MarketCandidate,
        now: chrono::DateTime<chrono::Utc>,
    ) -> (f64, f64, f64) {
        let lookback_start = now - chrono::Duration::hours(self.lookback_hours);
        let prices: Vec<f64> = candidate
            .price_history
            .iter()
            .filter(|p| p.timestamp >= lookback_start)
            .filter_map(|p| p.yes_price.to_f64())
            .collect();

        if prices.len() < 5 {
            return (0.0, 0.0, 1.0);
        }

        let (filtered_price, uncertainty) = self.run_kalman_filter(&prices);
        let current = candidate.current_yes_price.to_f64().unwrap_or(0.5);

        let innovation = current - filtered_price;
        let normalized_innovation = innovation / uncertainty.max(0.001);

        (
            filtered_price,
            normalized_innovation.clamp(-3.0, 3.0),
            uncertainty,
        )
    }
}

#[async_trait]
impl Scorer for KalmanPriceFilter {
    fn name(&self) -> &'static str {
        "KalmanPriceFilter"
    }

    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let (filtered, innovation, uncertainty) =
                    self.calculate_score(c, context.timestamp);
                let mut scored = MarketCandidate {
                    scores: c.scores.clone(),
                    ..Default::default()
                };
                scored.scores.insert("kalman_price".to_string(), filtered);
                scored
                    .scores
                    .insert("kalman_innovation".to_string(), innovation);
                scored
                    .scores
                    .insert("kalman_uncertainty".to_string(), uncertainty);
                scored
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        for key in ["kalman_price", "kalman_innovation", "kalman_uncertainty"] {
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
    fn test_kalman_default_config() {
        let filter = KalmanPriceFilter::default_config();
        assert_eq!(filter.lookback_hours, 24);
        assert_eq!(filter.process_noise, 0.001);
        assert_eq!(filter.measurement_noise, 0.01);
    }

    #[test]
    fn test_kalman_filter_empty() {
        let filter = KalmanPriceFilter::default_config();
        let (price, uncertainty) = filter.run_kalman_filter(&[]);
        assert_eq!(price, 0.5);
        assert_eq!(uncertainty, 1.0);
    }

    #[test]
    fn test_kalman_filter_single() {
        let filter = KalmanPriceFilter::default_config();
        let (price, _uncertainty) = filter.run_kalman_filter(&[0.6]);
        assert_eq!(price, 0.6);
    }
}
