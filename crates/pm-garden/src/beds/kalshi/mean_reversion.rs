//! Mean reversion bed - specimens that bet on return to mean
//!
//! These specimens identify overextended prices and trade for reversion.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pm_core::{MarketCandidate, Scorer, TradingContext};
use rust_decimal::prelude::ToPrimitive;

/// Simple z-score based mean reversion scorer
pub struct MeanReversionScorer {
    lookback_hours: i64,
}

impl MeanReversionScorer {
    pub fn new(lookback_hours: i64) -> Self {
        Self { lookback_hours }
    }

    fn calculate_deviation(
        candidate: &MarketCandidate,
        now: DateTime<Utc>,
        lookback_hours: i64,
    ) -> f64 {
        let lookback_start = now - chrono::Duration::hours(lookback_hours);
        let prices: Vec<f64> = candidate
            .price_history
            .iter()
            .filter(|p| p.timestamp >= lookback_start)
            .filter_map(|p| p.yes_price.to_f64())
            .collect();

        if prices.is_empty() {
            return 0.0;
        }

        let mean: f64 = prices.iter().sum::<f64>() / prices.len() as f64;
        let current = candidate.current_yes_price.to_f64().unwrap_or(0.5);
        let deviation = current - mean;

        // negative deviation = buy signal (price below mean)
        -deviation
    }
}

#[async_trait]
impl Scorer for MeanReversionScorer {
    fn name(&self) -> &'static str {
        "MeanReversionScorer"
    }

    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let reversion =
                    Self::calculate_deviation(c, context.timestamp, self.lookback_hours);
                let mut scored = MarketCandidate {
                    scores: c.scores.clone(),
                    ..Default::default()
                };
                scored
                    .scores
                    .insert("mean_reversion".to_string(), reversion);
                scored
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        if let Some(score) = scored.scores.get("mean_reversion") {
            candidate
                .scores
                .insert("mean_reversion".to_string(), *score);
        }
    }
}

/// Bollinger bands mean reversion scorer
///
/// Triggers when price touches statistical extremes (upper/lower bands).
pub struct BollingerMeanReversionScorer {
    lookback_hours: i64,
    num_std: f64,
}

impl BollingerMeanReversionScorer {
    pub fn new(lookback_hours: i64, num_std: f64) -> Self {
        Self {
            lookback_hours,
            num_std,
        }
    }

    pub fn default_config() -> Self {
        Self::new(24, 2.0)
    }

    fn calculate_bands(
        candidate: &MarketCandidate,
        now: DateTime<Utc>,
        lookback_hours: i64,
    ) -> Option<(f64, f64, f64)> {
        let lookback_start = now - chrono::Duration::hours(lookback_hours);
        let prices: Vec<f64> = candidate
            .price_history
            .iter()
            .filter(|p| p.timestamp >= lookback_start)
            .filter_map(|p| p.yes_price.to_f64())
            .collect();

        if prices.len() < 5 {
            return None;
        }

        let mean: f64 = prices.iter().sum::<f64>() / prices.len() as f64;
        let variance: f64 =
            prices.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / prices.len() as f64;
        let std = variance.sqrt();

        Some((mean, std, *prices.last().unwrap_or(&mean)))
    }

    fn calculate_score(
        candidate: &MarketCandidate,
        now: DateTime<Utc>,
        lookback_hours: i64,
        num_std: f64,
    ) -> (f64, f64) {
        let (mean, std, current) = match Self::calculate_bands(candidate, now, lookback_hours) {
            Some(v) => v,
            None => return (0.0, 0.0),
        };

        let upper_band = mean + num_std * std;
        let lower_band = mean - num_std * std;
        let band_width = upper_band - lower_band;

        if band_width < 0.001 {
            return (0.0, 0.0);
        }

        let position = (current - lower_band) / band_width;

        let score = if current >= upper_band {
            // price at/above upper band - sell signal (expect reversion down)
            -(current - upper_band) / std.max(0.001)
        } else if current <= lower_band {
            // price at/below lower band - buy signal (expect reversion up)
            (lower_band - current) / std.max(0.001)
        } else if current > mean {
            // above mean but within bands - weak sell signal
            -(position - 0.5) * 0.5
        } else {
            // below mean but within bands - weak buy signal
            (0.5 - position) * 0.5
        };

        let band_position = (position * 2.0 - 1.0).clamp(-1.0, 1.0);

        (score.clamp(-2.0, 2.0), band_position)
    }
}

#[async_trait]
impl Scorer for BollingerMeanReversionScorer {
    fn name(&self) -> &'static str {
        "BollingerMeanReversionScorer"
    }

    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let (score, band_pos) =
                    Self::calculate_score(c, context.timestamp, self.lookback_hours, self.num_std);
                let mut scored = MarketCandidate {
                    scores: c.scores.clone(),
                    ..Default::default()
                };
                scored
                    .scores
                    .insert("bollinger_reversion".to_string(), score);
                scored
                    .scores
                    .insert("bollinger_position".to_string(), band_pos);
                scored
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        for key in ["bollinger_reversion", "bollinger_position"] {
            if let Some(score) = scored.scores.get(key) {
                candidate.scores.insert(key.to_string(), *score);
            }
        }
    }
}
