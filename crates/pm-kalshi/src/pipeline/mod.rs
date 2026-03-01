//! Trading pipeline for Kalshi

mod selector;
mod trading_pipeline;

pub use selector::{ThresholdSelector, TopKSelector};
pub use trading_pipeline::TradingPipeline;

// Re-export sources
pub use crate::sources::{HistoricalMarketSource, LiveKalshiSource};

// Re-export garden specimens and filters
pub use pm_garden::{
    // ensemble scorers
    AdaptiveConfidenceScorer,
    // filters
    AlreadyPositionedFilter,
    BayesianEnsembleScorer,
    // mean reversion scorers
    BollingerMeanReversionScorer,
    CategoryFilter,
    CategoryWeightedScorer,
    CompositeFilter,
    EnsembleScorer,
    // advanced scorers
    EntropyScorer,
    GrangerCorrelationScorer,
    KalmanPriceFilter,
    LiquidityFilter,
    MeanReversionScorer,
    MomentumAccelerationScorer,
    // momentum scorers
    MomentumScorer,
    MultiTimeframeMomentumScorer,
    NormalizedScorer,
    // volume scorers
    OrderFlowScorer,
    PriceRangeFilter,
    RegimeAdaptiveScorer,
    RegimeDetector,
    SpreadFilter,
    TimeDecayScorer,
    TimeToCloseFilter,
    VPINScorer,
    VolatilityFilter,
    VolatilityScorer,
    VolumeScorer,
    WeightedScorer,
};
