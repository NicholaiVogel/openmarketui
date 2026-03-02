//! Ensemble bed - specimens that combine signals
//!
//! These specimens use multiple signals and adapt to market conditions.

use async_trait::async_trait;
use pm_core::{MarketCandidate, Scorer, TradingContext};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Scorer weights configuration for different market categories/regimes
#[derive(Clone)]
pub struct ScorerWeights {
    pub momentum: f64,
    pub mean_reversion: f64,
    pub volume: f64,
    pub time_decay: f64,
    pub order_flow: f64,
    pub bollinger: f64,
    pub mtf_momentum: f64,
}

impl Default for ScorerWeights {
    fn default() -> Self {
        Self {
            momentum: 0.2,
            mean_reversion: 0.2,
            volume: 0.15,
            time_decay: 0.1,
            order_flow: 0.15,
            bollinger: 0.1,
            mtf_momentum: 0.1,
        }
    }
}

impl ScorerWeights {
    pub fn politics() -> Self {
        Self {
            momentum: 0.35,
            mean_reversion: 0.1,
            volume: 0.1,
            time_decay: 0.1,
            order_flow: 0.15,
            bollinger: 0.05,
            mtf_momentum: 0.15,
        }
    }

    pub fn weather() -> Self {
        Self {
            momentum: 0.1,
            mean_reversion: 0.35,
            volume: 0.1,
            time_decay: 0.15,
            order_flow: 0.1,
            bollinger: 0.15,
            mtf_momentum: 0.05,
        }
    }

    pub fn sports() -> Self {
        Self {
            momentum: 0.2,
            mean_reversion: 0.1,
            volume: 0.15,
            time_decay: 0.1,
            order_flow: 0.3,
            bollinger: 0.05,
            mtf_momentum: 0.1,
        }
    }

    pub fn economics() -> Self {
        Self {
            momentum: 0.25,
            mean_reversion: 0.2,
            volume: 0.15,
            time_decay: 0.1,
            order_flow: 0.15,
            bollinger: 0.1,
            mtf_momentum: 0.05,
        }
    }

    pub fn compute_score(&self, candidate: &MarketCandidate) -> f64 {
        let get_score = |key: &str| candidate.scores.get(key).copied().unwrap_or(0.0);

        self.momentum * get_score("momentum")
            + self.mean_reversion * get_score("mean_reversion")
            + self.volume * get_score("volume")
            + self.time_decay * get_score("time_decay")
            + self.order_flow * get_score("order_flow")
            + self.bollinger * get_score("bollinger_reversion")
            + self.mtf_momentum * get_score("mtf_momentum")
    }
}

/// Category-aware weighted scorer
///
/// Applies different weights based on market category (politics, sports, etc.)
pub struct CategoryWeightedScorer {
    category_weights: HashMap<String, ScorerWeights>,
    default_weights: ScorerWeights,
}

impl CategoryWeightedScorer {
    pub fn new(
        category_weights: HashMap<String, ScorerWeights>,
        default_weights: ScorerWeights,
    ) -> Self {
        Self {
            category_weights,
            default_weights,
        }
    }

    pub fn with_defaults() -> Self {
        let mut category_weights = HashMap::new();
        category_weights.insert("politics".to_string(), ScorerWeights::politics());
        category_weights.insert("weather".to_string(), ScorerWeights::weather());
        category_weights.insert("sports".to_string(), ScorerWeights::sports());
        category_weights.insert("economics".to_string(), ScorerWeights::economics());
        category_weights.insert("financial".to_string(), ScorerWeights::economics());

        Self {
            category_weights,
            default_weights: ScorerWeights::default(),
        }
    }

    fn get_weights(&self, category: &str) -> &ScorerWeights {
        let lower = category.to_lowercase();
        self.category_weights
            .get(&lower)
            .unwrap_or(&self.default_weights)
    }
}

#[async_trait]
impl Scorer for CategoryWeightedScorer {
    fn name(&self) -> &'static str {
        "CategoryWeightedScorer"
    }

    async fn score(
        &self,
        _context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let weights = self.get_weights(&c.category);
                let weighted_score = weights.compute_score(c);
                MarketCandidate {
                    final_score: weighted_score,
                    ..Default::default()
                }
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        candidate.final_score = scored.final_score;
    }
}

/// Regime-adaptive scorer
///
/// Adjusts weights based on detected market regime (bull, bear, neutral, turning).
pub struct RegimeAdaptiveScorer {
    #[allow(dead_code)]
    default_weights: ScorerWeights,
}

impl RegimeAdaptiveScorer {
    pub fn new() -> Self {
        Self {
            default_weights: ScorerWeights::default(),
        }
    }

    fn get_regime_weights(&self, candidate: &MarketCandidate) -> ScorerWeights {
        let regime_score = candidate.scores.get("regime").copied().unwrap_or(0.0);
        let mom_regime = candidate
            .scores
            .get("momentum_regime")
            .copied()
            .unwrap_or(0.0);
        let turning_point = candidate
            .scores
            .get("turning_point")
            .copied()
            .unwrap_or(0.0);

        if turning_point.abs() > 0.0 {
            // turning point detected - be cautious
            ScorerWeights {
                momentum: 0.1,
                mean_reversion: 0.1,
                volume: 0.2,
                time_decay: 0.3,
                order_flow: 0.2,
                bollinger: 0.1,
                mtf_momentum: 0.0,
            }
        } else if regime_score > 0.5 || mom_regime > 0.5 {
            // bull regime - favor momentum
            ScorerWeights {
                momentum: 0.4,
                mean_reversion: 0.05,
                volume: 0.15,
                time_decay: 0.05,
                order_flow: 0.15,
                bollinger: 0.05,
                mtf_momentum: 0.15,
            }
        } else if regime_score < -0.5 || mom_regime < -0.5 {
            // bear regime - favor mean reversion
            ScorerWeights {
                momentum: 0.05,
                mean_reversion: 0.4,
                volume: 0.1,
                time_decay: 0.1,
                order_flow: 0.1,
                bollinger: 0.2,
                mtf_momentum: 0.05,
            }
        } else {
            // neutral regime - balanced approach
            ScorerWeights {
                momentum: 0.15,
                mean_reversion: 0.2,
                volume: 0.15,
                time_decay: 0.15,
                order_flow: 0.2,
                bollinger: 0.1,
                mtf_momentum: 0.05,
            }
        }
    }
}

impl Default for RegimeAdaptiveScorer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Scorer for RegimeAdaptiveScorer {
    fn name(&self) -> &'static str {
        "RegimeAdaptiveScorer"
    }

    async fn score(
        &self,
        _context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let weights = self.get_regime_weights(c);
                let weighted_score = weights.compute_score(c);
                MarketCandidate {
                    final_score: weighted_score,
                    ..Default::default()
                }
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        candidate.final_score = scored.final_score;
    }
}

/// Adaptive confidence scorer
///
/// Scales signal confidence based on estimation uncertainty and flow toxicity.
/// Uses kalman filter uncertainty, VPIN, and entropy for confidence scaling.
pub struct AdaptiveConfidenceScorer;

impl AdaptiveConfidenceScorer {
    pub fn new() -> Self {
        Self
    }

    fn calculate_confidence(&self, candidate: &MarketCandidate) -> f64 {
        let uncertainty = candidate
            .scores
            .get("kalman_uncertainty")
            .copied()
            .unwrap_or(1.0);
        let vpin = candidate.scores.get("vpin").copied().unwrap_or(0.0);
        let entropy = candidate.scores.get("entropy").copied().unwrap_or(0.5);

        let uncertainty_factor = 1.0 / (1.0 + uncertainty * 2.0);
        let vpin_factor = 1.0 - vpin * 0.2;
        let entropy_factor = 0.5 + (entropy * 0.5);

        (uncertainty_factor * vpin_factor * entropy_factor).clamp(0.4, 1.0)
    }

    fn get_regime_weights(&self, candidate: &MarketCandidate) -> ScorerWeights {
        let regime_score = candidate.scores.get("regime").copied().unwrap_or(0.0);
        let mom_regime = candidate
            .scores
            .get("momentum_regime")
            .copied()
            .unwrap_or(0.0);
        let turning_point = candidate
            .scores
            .get("turning_point")
            .copied()
            .unwrap_or(0.0);
        let informed_dir = candidate
            .scores
            .get("informed_direction")
            .copied()
            .unwrap_or(0.0);
        let vpin = candidate.scores.get("vpin").copied().unwrap_or(0.0);

        if vpin > 0.4 && informed_dir.abs() > 0.2 {
            // high informed trading - follow the flow
            ScorerWeights {
                momentum: 0.1,
                mean_reversion: 0.05,
                volume: 0.15,
                time_decay: 0.1,
                order_flow: 0.4,
                bollinger: 0.05,
                mtf_momentum: 0.15,
            }
        } else if turning_point.abs() > 0.2 {
            // turning point - be conservative
            ScorerWeights {
                momentum: 0.05,
                mean_reversion: 0.15,
                volume: 0.2,
                time_decay: 0.25,
                order_flow: 0.2,
                bollinger: 0.1,
                mtf_momentum: 0.05,
            }
        } else if regime_score > 0.5 || mom_regime > 0.5 {
            // bull regime
            ScorerWeights {
                momentum: 0.35,
                mean_reversion: 0.05,
                volume: 0.15,
                time_decay: 0.05,
                order_flow: 0.2,
                bollinger: 0.05,
                mtf_momentum: 0.15,
            }
        } else if regime_score < -0.5 || mom_regime < -0.5 {
            // bear regime
            ScorerWeights {
                momentum: 0.05,
                mean_reversion: 0.35,
                volume: 0.1,
                time_decay: 0.1,
                order_flow: 0.15,
                bollinger: 0.2,
                mtf_momentum: 0.05,
            }
        } else {
            // neutral
            ScorerWeights {
                momentum: 0.2,
                mean_reversion: 0.2,
                volume: 0.15,
                time_decay: 0.1,
                order_flow: 0.15,
                bollinger: 0.1,
                mtf_momentum: 0.1,
            }
        }
    }
}

impl Default for AdaptiveConfidenceScorer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Scorer for AdaptiveConfidenceScorer {
    fn name(&self) -> &'static str {
        "AdaptiveConfidenceScorer"
    }

    async fn score(
        &self,
        _context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let confidence = self.calculate_confidence(c);
                let weights = self.get_regime_weights(c);
                let raw_score = weights.compute_score(c);

                let momentum = c.scores.get("momentum").copied().unwrap_or(0.0).abs();
                let order_flow = c.scores.get("order_flow").copied().unwrap_or(0.0).abs();
                let informed_dir = c
                    .scores
                    .get("informed_direction")
                    .copied()
                    .unwrap_or(0.0)
                    .abs();

                let signal_strength = (momentum + order_flow + informed_dir) / 3.0;

                let confidence_boost = 0.7 + confidence * 0.3;
                let strength_boost = if signal_strength > 0.2 {
                    1.0 + (signal_strength - 0.2) * 0.3
                } else {
                    1.0
                };

                let final_score = raw_score * confidence_boost * strength_boost;

                MarketCandidate {
                    final_score,
                    scores: {
                        let mut s = HashMap::new();
                        s.insert("confidence".to_string(), confidence);
                        s.insert("signal_strength".to_string(), signal_strength);
                        s
                    },
                    ..Default::default()
                }
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        candidate.final_score = scored.final_score;
        if let Some(conf) = scored.scores.get("confidence") {
            candidate.scores.insert("confidence".to_string(), *conf);
        }
        if let Some(strength) = scored.scores.get("signal_strength") {
            candidate
                .scores
                .insert("signal_strength".to_string(), *strength);
        }
    }
}

/// Simple weighted scorer with configurable weights
pub struct WeightedScorer {
    weights: Vec<(String, f64)>,
}

impl WeightedScorer {
    pub fn new(weights: Vec<(String, f64)>) -> Self {
        Self { weights }
    }

    pub fn default_weights() -> Self {
        Self::new(vec![
            ("momentum".to_string(), 0.4),
            ("mean_reversion".to_string(), 0.3),
            ("volume".to_string(), 0.2),
            ("time_decay".to_string(), 0.1),
        ])
    }

    fn compute_weighted_score(&self, candidate: &MarketCandidate) -> f64 {
        self.weights
            .iter()
            .map(|(name, weight)| candidate.scores.get(name).copied().unwrap_or(0.0) * weight)
            .sum()
    }
}

#[async_trait]
impl Scorer for WeightedScorer {
    fn name(&self) -> &'static str {
        "WeightedScorer"
    }

    async fn score(
        &self,
        _context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let weighted_score = self.compute_weighted_score(c);
                MarketCandidate {
                    final_score: weighted_score,
                    ..Default::default()
                }
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        candidate.final_score = scored.final_score;
    }
}

/// Basic ensemble scorer with dynamic weight updates
pub struct EnsembleScorer {
    model_weights: Arc<Mutex<Vec<f64>>>,
    model_keys: Vec<String>,
}

impl EnsembleScorer {
    pub fn new(model_keys: Vec<String>, initial_weights: Vec<f64>) -> Self {
        assert_eq!(model_keys.len(), initial_weights.len());
        Self {
            model_weights: Arc::new(Mutex::new(initial_weights)),
            model_keys,
        }
    }

    pub fn default_ensemble() -> Self {
        Self::new(
            vec![
                "momentum".to_string(),
                "mean_reversion".to_string(),
                "bollinger_reversion".to_string(),
                "order_flow".to_string(),
                "mtf_momentum".to_string(),
            ],
            vec![0.25, 0.2, 0.2, 0.2, 0.15],
        )
    }

    pub fn update_weights(&self, new_weights: Vec<f64>) {
        let mut weights = self.model_weights.lock().unwrap();
        *weights = new_weights;
    }

    fn compute_score(&self, candidate: &MarketCandidate) -> f64 {
        let weights = self.model_weights.lock().unwrap();
        self.model_keys
            .iter()
            .zip(weights.iter())
            .map(|(key, weight)| candidate.scores.get(key).copied().unwrap_or(0.0) * weight)
            .sum()
    }
}

#[async_trait]
impl Scorer for EnsembleScorer {
    fn name(&self) -> &'static str {
        "EnsembleScorer"
    }

    async fn score(
        &self,
        _context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let scored = candidates
            .iter()
            .map(|c| {
                let ensemble_score = self.compute_score(c);
                MarketCandidate {
                    final_score: ensemble_score,
                    ..Default::default()
                }
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        candidate.final_score = scored.final_score;
    }
}

/// Bayesian ensemble scorer with dirichlet prior for weight updates
pub struct BayesianEnsembleScorer {
    weights: Arc<Mutex<Vec<f64>>>,
    model_keys: Vec<String>,
    alpha_prior: f64,
    accuracy_history: Arc<Mutex<HashMap<String, Vec<f64>>>>,
    history_size: usize,
}

impl BayesianEnsembleScorer {
    pub fn new(model_keys: Vec<String>, alpha_prior: f64, history_size: usize) -> Self {
        let initial_weights = vec![1.0; model_keys.len()];
        let mut accuracy_history = HashMap::new();
        for key in &model_keys {
            accuracy_history.insert(key.clone(), Vec::new());
        }

        Self {
            weights: Arc::new(Mutex::new(initial_weights)),
            model_keys,
            alpha_prior,
            accuracy_history: Arc::new(Mutex::new(accuracy_history)),
            history_size,
        }
    }

    pub fn update_accuracy(&self, model_name: &str, accuracy: f64) {
        let mut history = self.accuracy_history.lock().unwrap();
        let entry = history
            .entry(model_name.to_string())
            .or_insert_with(Vec::new);
        entry.push(accuracy);
        if entry.len() > self.history_size {
            entry.remove(0);
        }
    }

    fn update_weights(&self) {
        let mut weights = self.weights.lock().unwrap();
        let history = self.accuracy_history.lock().unwrap();

        let mut total_alpha = 0.0;
        for (i, key) in self.model_keys.iter().enumerate() {
            if let Some(acc_history) = history.get(key) {
                if !acc_history.is_empty() {
                    let avg_acc = acc_history.iter().sum::<f64>() / acc_history.len() as f64;
                    let alpha = self.alpha_prior + avg_acc;
                    total_alpha += alpha;
                    weights[i] = alpha;
                }
            }
        }

        if total_alpha > 0.0 {
            for weight in weights.iter_mut() {
                *weight /= total_alpha;
            }
        }
    }

    fn compute_score(&self, candidate: &MarketCandidate) -> f64 {
        let weights = self.weights.lock().unwrap();
        self.model_keys
            .iter()
            .zip(weights.iter())
            .map(|(key, weight)| candidate.scores.get(key).copied().unwrap_or(0.0) * weight)
            .sum()
    }
}

#[async_trait]
impl Scorer for BayesianEnsembleScorer {
    fn name(&self) -> &'static str {
        "BayesianEnsembleScorer"
    }

    async fn score(
        &self,
        _context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        self.update_weights();

        let scored = candidates
            .iter()
            .map(|c| {
                let ensemble_score = self.compute_score(c);
                let mut scored = MarketCandidate {
                    scores: c.scores.clone(),
                    ..Default::default()
                };
                scored
                    .scores
                    .insert("bayesian_ensemble".to_string(), ensemble_score);
                scored
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        if let Some(score) = scored.scores.get("bayesian_ensemble") {
            candidate
                .scores
                .insert("bayesian_ensemble".to_string(), *score);
        }
    }
}

/// Rolling statistics for z-score normalization
#[derive(Debug, Clone)]
pub struct RollingStats {
    values: Vec<f64>,
    max_size: usize,
    sum: f64,
    sum_sq: f64,
}

impl RollingStats {
    pub fn new(max_size: usize) -> Self {
        Self {
            values: Vec::with_capacity(max_size),
            max_size,
            sum: 0.0,
            sum_sq: 0.0,
        }
    }

    pub fn push(&mut self, value: f64) {
        if !value.is_finite() {
            return;
        }

        if self.values.len() >= self.max_size {
            let old = self.values.remove(0);
            self.sum -= old;
            self.sum_sq -= old * old;
        }

        self.values.push(value);
        self.sum += value;
        self.sum_sq += value * value;
    }

    pub fn push_batch(&mut self, values: &[f64]) {
        for &v in values {
            self.push(v);
        }
    }

    pub fn mean(&self) -> f64 {
        if self.values.is_empty() {
            0.0
        } else {
            self.sum / self.values.len() as f64
        }
    }

    pub fn std(&self) -> f64 {
        let n = self.values.len();
        if n < 2 {
            return 1.0;
        }

        let mean = self.mean();
        let variance = (self.sum_sq / n as f64) - (mean * mean);
        variance.max(0.0).sqrt()
    }

    pub fn normalize(&self, value: f64) -> f64 {
        let std = self.std().max(0.001);
        (value - self.mean()) / std
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn is_ready(&self) -> bool {
        self.values.len() >= self.max_size / 4
    }
}

/// Wrapper that normalizes any scorer's output to z-scores
pub struct NormalizedScorer<S> {
    inner: S,
    score_key: String,
    stats: Arc<Mutex<RollingStats>>,
}

impl<S> NormalizedScorer<S> {
    pub fn new(inner: S, score_key: &str, history_size: usize) -> Self {
        Self {
            inner,
            score_key: score_key.to_string(),
            stats: Arc::new(Mutex::new(RollingStats::new(history_size))),
        }
    }
}

#[async_trait]
impl<S: Scorer + Send + Sync> Scorer for NormalizedScorer<S> {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let raw_scored = self.inner.score(context, candidates).await?;

        let raw_scores: Vec<f64> = raw_scored
            .iter()
            .filter_map(|c| c.scores.get(&self.score_key).copied())
            .collect();

        {
            let mut stats = self.stats.lock().unwrap();
            stats.push_batch(&raw_scores);
        }

        let stats = self.stats.lock().unwrap();
        let normalized = raw_scored
            .into_iter()
            .map(|mut c| {
                if let Some(&raw) = c.scores.get(&self.score_key) {
                    let z = if stats.is_ready() {
                        stats.normalize(raw)
                    } else {
                        raw
                    };
                    c.scores.insert(self.score_key.clone(), z);
                }
                c
            })
            .collect();

        Ok(normalized)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        self.inner.update(candidate, scored);
    }
}
