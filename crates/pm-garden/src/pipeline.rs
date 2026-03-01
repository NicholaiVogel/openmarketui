//! Trading pipeline - the irrigation system
//!
//! The pipeline waters specimens with data, applies filters,
//! scores candidates, and selects the best opportunities.

use pm_core::{
    Fill, Filter, OrderExecutor, PipelineResult, Scorer, Selector, Signal, Source, TradingContext,
};
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};

/// The main trading pipeline
///
/// Orchestrates the flow: Source -> Filters -> Scorers -> Selector -> Executor
pub struct TradingPipeline {
    sources: Vec<Arc<dyn Source>>,
    filters: Vec<Arc<dyn Filter>>,
    scorers: Vec<Arc<dyn Scorer>>,
    selector: Arc<dyn Selector>,
    executor: Arc<dyn OrderExecutor>,
}

impl TradingPipeline {
    pub fn new(
        sources: Vec<Arc<dyn Source>>,
        filters: Vec<Arc<dyn Filter>>,
        scorers: Vec<Arc<dyn Scorer>>,
        selector: Arc<dyn Selector>,
        executor: Arc<dyn OrderExecutor>,
    ) -> Self {
        Self {
            sources,
            filters,
            scorers,
            selector,
            executor,
        }
    }

    /// Run a single tick of the pipeline
    #[instrument(skip(self, context), fields(timestamp = %context.timestamp))]
    pub async fn tick(&self, context: &TradingContext) -> Result<PipelineResult, String> {
        let context_arc = Arc::new(context.clone());

        // gather candidates from all sources
        let mut all_candidates = Vec::new();
        for source in &self.sources {
            if !source.enable(context) {
                debug!(source = source.name(), "source disabled for this tick");
                continue;
            }

            match source.get_candidates(context).await {
                Ok(candidates) => {
                    debug!(
                        source = source.name(),
                        count = candidates.len(),
                        "gathered candidates"
                    );
                    all_candidates.extend(candidates);
                }
                Err(e) => {
                    warn!(source = source.name(), error = %e, "failed to get candidates");
                }
            }
        }

        if all_candidates.is_empty() {
            return Ok(PipelineResult {
                retrieved_candidates: vec![],
                filtered_candidates: vec![],
                selected_candidates: vec![],
                context: context_arc,
            });
        }

        let retrieved_candidates = all_candidates.clone();

        // apply filters
        let mut candidates = all_candidates;

        for filter in &self.filters {
            let result = filter.filter(context, candidates).await?;

            debug!(
                filter = filter.name(),
                kept = result.kept.len(),
                removed = result.removed.len(),
                "applied filter"
            );

            candidates = result.kept;

            if candidates.is_empty() {
                debug!("all candidates filtered out");
                return Ok(PipelineResult {
                    retrieved_candidates,
                    filtered_candidates: vec![],
                    selected_candidates: vec![],
                    context: context_arc,
                });
            }
        }

        let filtered_candidates = candidates.clone();

        // apply scorers
        for scorer in &self.scorers {
            let scored = scorer.score(context, &candidates).await?;

            // update candidates with scores
            for (candidate, scored_result) in candidates.iter_mut().zip(scored.into_iter()) {
                scorer.update(candidate, scored_result);
            }

            debug!(
                scorer = scorer.name(),
                count = candidates.len(),
                "applied scorer"
            );
        }

        // select best opportunities
        let selected_candidates = self.selector.select(context, candidates);

        info!(
            initial = retrieved_candidates.len(),
            filtered = filtered_candidates.len(),
            selected = selected_candidates.len(),
            "pipeline tick complete"
        );

        Ok(PipelineResult {
            retrieved_candidates,
            filtered_candidates,
            selected_candidates,
            context: context_arc,
        })
    }

    /// Execute signals through the executor
    pub async fn execute_signals(
        &self,
        context: &TradingContext,
        signals: Vec<Signal>,
    ) -> Vec<Fill> {
        let mut fills = Vec::new();

        for signal in signals {
            if let Some(fill) = self.executor.execute_signal(&signal, context).await {
                info!(
                    ticker = %signal.ticker,
                    side = ?signal.side,
                    quantity = signal.quantity,
                    "signal executed"
                );
                fills.push(fill);
            } else {
                debug!(ticker = %signal.ticker, "signal skipped or failed");
            }
        }

        fills
    }

    /// Generate and execute entry signals from pipeline result
    pub async fn execute_entries(
        &self,
        context: &TradingContext,
        result: &PipelineResult,
    ) -> Vec<Fill> {
        let signals = self
            .executor
            .generate_signals(&result.selected_candidates, context);
        self.execute_signals(context, signals).await
    }

    /// Run a full cycle: tick + execute entries
    pub async fn run_cycle(&self, context: &TradingContext) -> Result<Vec<Fill>, String> {
        let result = self.tick(context).await?;
        let fills = self.execute_entries(context, &result).await;
        Ok(fills)
    }
}

/// Builder for constructing pipelines
pub struct PipelineBuilder {
    sources: Vec<Arc<dyn Source>>,
    filters: Vec<Arc<dyn Filter>>,
    scorers: Vec<Arc<dyn Scorer>>,
    selector: Option<Arc<dyn Selector>>,
    executor: Option<Arc<dyn OrderExecutor>>,
}

impl PipelineBuilder {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            filters: Vec::new(),
            scorers: Vec::new(),
            selector: None,
            executor: None,
        }
    }

    pub fn add_source(mut self, source: Arc<dyn Source>) -> Self {
        self.sources.push(source);
        self
    }

    pub fn add_filter(mut self, filter: Arc<dyn Filter>) -> Self {
        self.filters.push(filter);
        self
    }

    pub fn add_scorer(mut self, scorer: Arc<dyn Scorer>) -> Self {
        self.scorers.push(scorer);
        self
    }

    pub fn selector(mut self, selector: Arc<dyn Selector>) -> Self {
        self.selector = Some(selector);
        self
    }

    pub fn executor(mut self, executor: Arc<dyn OrderExecutor>) -> Self {
        self.executor = Some(executor);
        self
    }

    pub fn build(self) -> Result<TradingPipeline, String> {
        let selector = self
            .selector
            .ok_or_else(|| "selector is required".to_string())?;
        let executor = self
            .executor
            .ok_or_else(|| "executor is required".to_string())?;

        if self.sources.is_empty() {
            return Err("at least one source is required".to_string());
        }

        Ok(TradingPipeline::new(
            self.sources,
            self.filters,
            self.scorers,
            selector,
            executor,
        ))
    }
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}
