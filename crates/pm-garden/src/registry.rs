//! Specimen registry - the seed catalog
//!
//! Dynamic registration and lookup of specimens (scorers, filters).
//! Allows loading specimens from config and runtime registration.

use pm_core::{Filter, Scorer};
use std::collections::HashMap;
use std::sync::Arc;

/// Factory function for creating scorers
pub type ScorerFactory = Box<dyn Fn(&toml::Value) -> Result<Arc<dyn Scorer>, String> + Send + Sync>;

/// Factory function for creating filters
pub type FilterFactory = Box<dyn Fn(&toml::Value) -> Result<Arc<dyn Filter>, String> + Send + Sync>;

/// Registry for dynamically loading specimens
pub struct SpecimenRegistry {
    scorers: HashMap<String, ScorerFactory>,
    filters: HashMap<String, FilterFactory>,
}

impl SpecimenRegistry {
    pub fn new() -> Self {
        Self {
            scorers: HashMap::new(),
            filters: HashMap::new(),
        }
    }

    /// Register a scorer factory
    pub fn register_scorer<F>(&mut self, name: &str, factory: F)
    where
        F: Fn(&toml::Value) -> Result<Arc<dyn Scorer>, String> + Send + Sync + 'static,
    {
        self.scorers.insert(name.to_string(), Box::new(factory));
    }

    /// Register a filter factory
    pub fn register_filter<F>(&mut self, name: &str, factory: F)
    where
        F: Fn(&toml::Value) -> Result<Arc<dyn Filter>, String> + Send + Sync + 'static,
    {
        self.filters.insert(name.to_string(), Box::new(factory));
    }

    /// Create a scorer from config
    pub fn create_scorer(
        &self,
        name: &str,
        config: &toml::Value,
    ) -> Result<Arc<dyn Scorer>, String> {
        let factory = self
            .scorers
            .get(name)
            .ok_or_else(|| format!("unknown scorer: {}", name))?;
        factory(config)
    }

    /// Create a filter from config
    pub fn create_filter(
        &self,
        name: &str,
        config: &toml::Value,
    ) -> Result<Arc<dyn Filter>, String> {
        let factory = self
            .filters
            .get(name)
            .ok_or_else(|| format!("unknown filter: {}", name))?;
        factory(config)
    }

    /// List all registered scorers
    pub fn list_scorers(&self) -> Vec<&str> {
        self.scorers.keys().map(|s| s.as_str()).collect()
    }

    /// List all registered filters
    pub fn list_filters(&self) -> Vec<&str> {
        self.filters.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a scorer is registered
    pub fn has_scorer(&self, name: &str) -> bool {
        self.scorers.contains_key(name)
    }

    /// Check if a filter is registered
    pub fn has_filter(&self, name: &str) -> bool {
        self.filters.contains_key(name)
    }
}

impl Default for SpecimenRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a registry with all default Kalshi specimens registered
pub fn default_kalshi_registry() -> SpecimenRegistry {
    use crate::beds::kalshi::*;

    let mut registry = SpecimenRegistry::new();

    // momentum bed
    registry.register_scorer("momentum", |config| {
        let lookback = config
            .get("lookback_hours")
            .and_then(|v| v.as_integer())
            .unwrap_or(6);
        Ok(Arc::new(MomentumScorer::new(lookback)))
    });

    registry.register_scorer("mtf_momentum", |config| {
        let windows: Vec<i64> = config
            .get("windows")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_integer()).collect())
            .unwrap_or_else(|| vec![1, 4, 12, 24]);
        Ok(Arc::new(MultiTimeframeMomentumScorer::new(windows)))
    });

    registry.register_scorer("time_decay", |_config| Ok(Arc::new(TimeDecayScorer::new())));

    // mean reversion bed
    registry.register_scorer("mean_reversion", |config| {
        let lookback = config
            .get("lookback_hours")
            .and_then(|v| v.as_integer())
            .unwrap_or(24);
        Ok(Arc::new(MeanReversionScorer::new(lookback)))
    });

    registry.register_scorer("bollinger", |config| {
        let lookback = config
            .get("lookback_hours")
            .and_then(|v| v.as_integer())
            .unwrap_or(24);
        let num_std = config
            .get("num_std")
            .and_then(|v| v.as_float())
            .unwrap_or(2.0);
        Ok(Arc::new(BollingerMeanReversionScorer::new(
            lookback, num_std,
        )))
    });

    // volume bed
    registry.register_scorer("volume", |config| {
        let lookback = config
            .get("lookback_hours")
            .and_then(|v| v.as_integer())
            .unwrap_or(24);
        Ok(Arc::new(VolumeScorer::new(lookback)))
    });

    registry.register_scorer("order_flow", |_config| Ok(Arc::new(OrderFlowScorer::new())));

    registry.register_scorer("vpin", |config| {
        let bucket_size = config
            .get("bucket_size")
            .and_then(|v| v.as_integer())
            .unwrap_or(50) as u64;
        let num_buckets = config
            .get("num_buckets")
            .and_then(|v| v.as_integer())
            .unwrap_or(20) as usize;
        Ok(Arc::new(VPINScorer::new(bucket_size, num_buckets)))
    });

    // ensemble bed
    registry.register_scorer("category_weighted", |_config| {
        Ok(Arc::new(CategoryWeightedScorer::with_defaults()))
    });

    registry.register_scorer("regime_adaptive", |_config| {
        Ok(Arc::new(RegimeAdaptiveScorer::new()))
    });

    registry.register_scorer("adaptive_confidence", |_config| {
        Ok(Arc::new(AdaptiveConfidenceScorer::new()))
    });

    registry.register_scorer("weighted", |config| {
        let weights: Vec<(String, f64)> = config
            .get("weights")
            .and_then(|v| v.as_table())
            .map(|t| {
                t.iter()
                    .filter_map(|(k, v)| v.as_float().map(|f| (k.clone(), f)))
                    .collect()
            })
            .unwrap_or_else(|| {
                vec![
                    ("momentum".to_string(), 0.4),
                    ("mean_reversion".to_string(), 0.3),
                    ("volume".to_string(), 0.2),
                    ("time_decay".to_string(), 0.1),
                ]
            });
        Ok(Arc::new(WeightedScorer::new(weights)))
    });

    // filters
    registry.register_filter("liquidity", |config| {
        let min_volume = config
            .get("min_volume_24h")
            .and_then(|v| v.as_integer())
            .unwrap_or(100) as u64;
        Ok(Arc::new(crate::filters::LiquidityFilter::new(min_volume)))
    });

    registry.register_filter("time_to_close", |config| {
        let min_hours = config
            .get("min_hours")
            .and_then(|v| v.as_integer())
            .unwrap_or(2);
        let max_hours = config.get("max_hours").and_then(|v| v.as_integer());
        Ok(Arc::new(crate::filters::TimeToCloseFilter::new(
            min_hours, max_hours,
        )))
    });

    registry.register_filter("already_positioned", |config| {
        let max_pos = config
            .get("max_position_per_market")
            .and_then(|v| v.as_integer())
            .unwrap_or(100) as u64;
        Ok(Arc::new(crate::filters::AlreadyPositionedFilter::new(
            max_pos,
        )))
    });

    registry.register_filter("category", |config| {
        if let Some(whitelist) = config.get("whitelist").and_then(|v| v.as_array()) {
            let categories: Vec<String> = whitelist
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            Ok(Arc::new(crate::filters::CategoryFilter::whitelist(
                categories,
            )))
        } else if let Some(blacklist) = config.get("blacklist").and_then(|v| v.as_array()) {
            let categories: Vec<String> = blacklist
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            Ok(Arc::new(crate::filters::CategoryFilter::blacklist(
                categories,
            )))
        } else {
            Err("category filter requires whitelist or blacklist".to_string())
        }
    });

    registry.register_filter("price_range", |config| {
        let min_price = config
            .get("min_price")
            .and_then(|v| v.as_float())
            .unwrap_or(0.1);
        let max_price = config
            .get("max_price")
            .and_then(|v| v.as_float())
            .unwrap_or(0.9);
        Ok(Arc::new(crate::filters::PriceRangeFilter::new(
            min_price, max_price,
        )))
    });

    registry.register_filter("spread", |config| {
        let max_spread = config
            .get("max_spread")
            .and_then(|v| v.as_float())
            .unwrap_or(0.05);
        Ok(Arc::new(crate::filters::SpreadFilter::new(max_spread)))
    });

    registry
}

/// Specimen status for monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecimenStatus {
    /// Actively producing signals
    Blooming,
    /// Disabled but preserved
    Dormant,
    /// Removed from the garden
    Pruned,
}

impl std::fmt::Display for SpecimenStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecimenStatus::Blooming => write!(f, "blooming"),
            SpecimenStatus::Dormant => write!(f, "dormant"),
            SpecimenStatus::Pruned => write!(f, "pruned"),
        }
    }
}

/// Metadata about a registered specimen
#[derive(Debug, Clone)]
pub struct SpecimenInfo {
    pub name: String,
    pub bed: String,
    pub status: SpecimenStatus,
    pub weight: f64,
}

impl SpecimenInfo {
    pub fn new(name: &str, bed: &str) -> Self {
        Self {
            name: name.to_string(),
            bed: bed.to_string(),
            status: SpecimenStatus::Blooming,
            weight: 1.0,
        }
    }

    pub fn with_status(mut self, status: SpecimenStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }
}
