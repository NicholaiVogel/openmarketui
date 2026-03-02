//! Core traits for the trading pipeline
//!
//! These define the interfaces for pluggable components:
//! - Source: Where market candidates come from (the water)
//! - Filter: Which candidates to keep (the immune system)
//! - Scorer: How to evaluate candidates (the specimens)
//! - Selector: Which scored candidates to trade (the harvest selection)
//! - OrderExecutor: How to execute trades (the harvester)

use crate::{ExitSignal, Fill, MarketCandidate, Signal, TradingContext};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Result of running the full pipeline
pub struct PipelineResult {
    pub retrieved_candidates: Vec<MarketCandidate>,
    pub filtered_candidates: Vec<MarketCandidate>,
    pub selected_candidates: Vec<MarketCandidate>,
    pub context: Arc<TradingContext>,
}

/// Result from a filter stage
pub struct FilterResult {
    pub kept: Vec<MarketCandidate>,
    pub removed: Vec<MarketCandidate>,
}

/// Source of market candidates (watering the garden)
#[async_trait]
pub trait Source: Send + Sync {
    /// Human-readable name for logging
    fn name(&self) -> &'static str;

    /// Whether this source is enabled for the current context
    fn enable(&self, _context: &TradingContext) -> bool {
        true
    }

    /// Fetch market candidates from this source
    async fn get_candidates(
        &self,
        context: &TradingContext,
    ) -> Result<Vec<MarketCandidate>, String>;
}

/// Filter for removing unsuitable candidates (garden immune system)
#[async_trait]
pub trait Filter: Send + Sync {
    fn name(&self) -> &'static str;

    fn enable(&self, _context: &TradingContext) -> bool {
        true
    }

    async fn filter(
        &self,
        context: &TradingContext,
        candidates: Vec<MarketCandidate>,
    ) -> Result<FilterResult, String>;
}

/// Scorer for evaluating candidates (specimens in the garden)
///
/// Each scorer produces one or more score keys that get stored
/// in the candidate's `scores` HashMap.
#[async_trait]
pub trait Scorer: Send + Sync {
    fn name(&self) -> &'static str;

    fn enable(&self, _context: &TradingContext) -> bool {
        true
    }

    /// Score the candidates, returning scored copies
    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String>;

    /// Update a candidate with scores from a scored copy
    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate);

    /// Batch update helper
    fn update_all(&self, candidates: &mut [MarketCandidate], scored: Vec<MarketCandidate>) {
        for (c, s) in candidates.iter_mut().zip(scored) {
            self.update(c, s);
        }
    }
}

/// Selector for choosing which candidates to trade (harvest selection)
pub trait Selector: Send + Sync {
    fn name(&self) -> &'static str;

    fn enable(&self, _context: &TradingContext) -> bool {
        true
    }

    fn select(
        &self,
        context: &TradingContext,
        candidates: Vec<MarketCandidate>,
    ) -> Vec<MarketCandidate>;
}

/// Executor for placing orders (the harvester)
#[async_trait]
pub trait OrderExecutor: Send + Sync {
    /// Execute a signal, returning a fill if successful
    async fn execute_signal(&self, signal: &Signal, context: &TradingContext) -> Option<Fill>;

    /// Generate entry signals from scored candidates
    fn generate_signals(
        &self,
        candidates: &[MarketCandidate],
        context: &TradingContext,
    ) -> Vec<Signal>;

    /// Generate exit signals for current positions
    fn generate_exit_signals(
        &self,
        context: &TradingContext,
        candidate_scores: &HashMap<String, f64>,
    ) -> Vec<ExitSignal>;
}
