//! Market selectors for the trading pipeline

use pm_core::{MarketCandidate, Selector, TradingContext};

pub struct TopKSelector {
    k: usize,
}

impl TopKSelector {
    pub fn new(k: usize) -> Self {
        Self { k }
    }
}

impl Selector for TopKSelector {
    fn name(&self) -> &'static str {
        "TopKSelector"
    }

    fn select(
        &self,
        _context: &TradingContext,
        mut candidates: Vec<MarketCandidate>,
    ) -> Vec<MarketCandidate> {
        candidates.sort_by(|a, b| {
            b.final_score
                .partial_cmp(&a.final_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        candidates.truncate(self.k);
        candidates
    }
}

pub struct ThresholdSelector {
    min_score: f64,
    max_candidates: Option<usize>,
}

impl ThresholdSelector {
    pub fn new(min_score: f64, max_candidates: Option<usize>) -> Self {
        Self {
            min_score,
            max_candidates,
        }
    }
}

impl Selector for ThresholdSelector {
    fn name(&self) -> &'static str {
        "ThresholdSelector"
    }

    fn select(
        &self,
        _context: &TradingContext,
        mut candidates: Vec<MarketCandidate>,
    ) -> Vec<MarketCandidate> {
        candidates.retain(|c| c.final_score >= self.min_score);
        candidates.sort_by(|a, b| {
            b.final_score
                .partial_cmp(&a.final_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if let Some(max) = self.max_candidates {
            candidates.truncate(max);
        }

        candidates
    }
}
