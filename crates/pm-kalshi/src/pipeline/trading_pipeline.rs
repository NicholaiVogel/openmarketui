//! Trading pipeline orchestration

use pm_core::{Filter, MarketCandidate, PipelineResult, Scorer, Selector, Source, TradingContext};
use std::sync::Arc;
use tracing::{error, info};

pub struct TradingPipeline {
    sources: Vec<Box<dyn Source>>,
    filters: Vec<Box<dyn Filter>>,
    scorers: Vec<Box<dyn Scorer>>,
    selector: Box<dyn Selector>,
    result_size: usize,
}

impl TradingPipeline {
    pub fn new(
        sources: Vec<Box<dyn Source>>,
        filters: Vec<Box<dyn Filter>>,
        scorers: Vec<Box<dyn Scorer>>,
        selector: Box<dyn Selector>,
        result_size: usize,
    ) -> Self {
        Self {
            sources,
            filters,
            scorers,
            selector,
            result_size,
        }
    }

    pub async fn execute(&self, context: TradingContext) -> PipelineResult {
        let request_id = context.request_id().to_string();

        let candidates = self.fetch_candidates(&context).await;
        info!(
            request_id = %request_id,
            candidates = candidates.len(),
            "fetched candidates"
        );

        let (kept, filtered) = self.filter(&context, candidates.clone()).await;
        info!(
            request_id = %request_id,
            kept = kept.len(),
            filtered = filtered.len(),
            "filtered candidates"
        );

        let scored = self.score(&context, kept).await;

        let mut selected = self.select(&context, scored);
        selected.truncate(self.result_size);

        info!(
            request_id = %request_id,
            selected = selected.len(),
            "selected candidates"
        );

        PipelineResult {
            retrieved_candidates: candidates,
            filtered_candidates: filtered,
            selected_candidates: selected,
            context: Arc::new(context),
        }
    }

    async fn fetch_candidates(&self, context: &TradingContext) -> Vec<MarketCandidate> {
        let mut all_candidates = Vec::new();

        for source in self.sources.iter().filter(|s| s.enable(context)) {
            match source.get_candidates(context).await {
                Ok(mut candidates) => {
                    info!(
                        source = source.name(),
                        count = candidates.len(),
                        "source returned candidates"
                    );
                    all_candidates.append(&mut candidates);
                }
                Err(e) => {
                    error!(source = source.name(), error = %e, "source failed");
                }
            }
        }

        all_candidates
    }

    async fn filter(
        &self,
        context: &TradingContext,
        mut candidates: Vec<MarketCandidate>,
    ) -> (Vec<MarketCandidate>, Vec<MarketCandidate>) {
        let mut all_removed = Vec::new();

        for filter in self.filters.iter().filter(|f| f.enable(context)) {
            let backup = candidates.clone();
            match filter.filter(context, candidates).await {
                Ok(result) => {
                    info!(
                        filter = filter.name(),
                        kept = result.kept.len(),
                        removed = result.removed.len(),
                        "filter applied"
                    );
                    candidates = result.kept;
                    all_removed.extend(result.removed);
                }
                Err(e) => {
                    error!(filter = filter.name(), error = %e, "filter failed");
                    candidates = backup;
                }
            }
        }

        (candidates, all_removed)
    }

    async fn score(
        &self,
        context: &TradingContext,
        mut candidates: Vec<MarketCandidate>,
    ) -> Vec<MarketCandidate> {
        let expected_len = candidates.len();

        for scorer in self.scorers.iter().filter(|s| s.enable(context)) {
            match scorer.score(context, &candidates).await {
                Ok(scored) => {
                    if scored.len() == expected_len {
                        scorer.update_all(&mut candidates, scored);
                    } else {
                        error!(
                            scorer = scorer.name(),
                            expected = expected_len,
                            got = scored.len(),
                            "scorer returned wrong number of candidates"
                        );
                    }
                }
                Err(e) => {
                    error!(scorer = scorer.name(), error = %e, "scorer failed");
                }
            }
        }

        candidates
    }

    fn select(
        &self,
        context: &TradingContext,
        candidates: Vec<MarketCandidate>,
    ) -> Vec<MarketCandidate> {
        if self.selector.enable(context) {
            self.selector.select(context, candidates)
        } else {
            candidates
        }
    }
}
