//! pm-garden: The garden of trading specimens
//!
//! Specimens (scorers) are organized into beds by family:
//! - momentum: Trend-following strategies
//! - mean_reversion: Mean reversion strategies
//! - volume: Volume and flow analysis
//! - ensemble: Combination and meta-strategies
//! - advanced: Battle-tested strategies (Kalman, entropy, regime detection)
//!
//! The garden also contains filters (the immune system) that
//! protect against bad trades.

pub mod beds;
pub mod filters;
pub mod pipeline;
pub mod registry;

// Re-export commonly used items
pub use beds::kalshi;
pub use pipeline::{PipelineBuilder, TradingPipeline};
pub use registry::{default_kalshi_registry, SpecimenInfo, SpecimenRegistry, SpecimenStatus};

// Re-export all filters
pub use filters::{
    AlreadyPositionedFilter, CategoryFilter, CompositeFilter, LiquidityFilter, PriceRangeFilter,
    SpreadFilter, TimeToCloseFilter, VolatilityFilter,
};

// Re-export Kalshi bed specimens
pub use beds::kalshi::{
    // ensemble
    AdaptiveConfidenceScorer,
    BayesianEnsembleScorer,
    // mean reversion
    BollingerMeanReversionScorer,
    CategoryWeightedScorer,
    EnsembleScorer,
    MeanReversionScorer,
    // momentum
    MomentumScorer,
    MultiTimeframeMomentumScorer,
    NormalizedScorer,
    // volume
    OrderFlowScorer,
    RegimeAdaptiveScorer,
    RollingStats,
    ScorerWeights,
    TimeDecayScorer,
    VPINScorer,
    VolumeScorer,
    WeightedScorer,
};

// Re-export advanced bed specimens
pub use beds::advanced::{
    EntropyScorer, GrangerCorrelationScorer, KalmanPriceFilter, MomentumAccelerationScorer,
    RegimeDetector, VolatilityScorer,
};
