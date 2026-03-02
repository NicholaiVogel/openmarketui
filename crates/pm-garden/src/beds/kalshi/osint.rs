//! OSINT intelligence bed - specimens that trade on geopolitical signals
//!
//! Reads structured signal files produced by the Python OSINT pipeline
//! (compost/osint/) and scores market candidates based on signal relevance,
//! urgency, and conviction.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pm_core::{MarketCandidate, Scorer, TradingContext};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, error, warn};

/// Urgency levels from the OSINT pipeline, with associated score multipliers.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Urgency {
    Breaking,
    High,
    Medium,
    Low,
}

impl Urgency {
    fn multiplier(&self) -> f64 {
        match self {
            Urgency::Breaking => 1.4,
            Urgency::High => 1.2,
            Urgency::Medium => 1.0,
            Urgency::Low => 0.85,
        }
    }
}

/// Signal categories from the OSINT pipeline.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Geopolitics,
    Economic,
    Military,
    Political,
    Climate,
}

/// A structured intelligence signal produced by the Python OSINT pipeline.
#[derive(Debug, Clone, Deserialize)]
pub struct OsintSignal {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub source_channel: String,
    pub urgency: Urgency,
    pub category: Category,
    pub entities: Vec<String>,
    pub summary: String,
    pub raw_text: String,
    pub relevant_tickers: Vec<String>,
    pub conviction: f64,
    pub themes: Vec<String>,
}

/// Scorer that evaluates market candidates against OSINT intelligence signals.
///
/// Signal matching works in three layers:
/// 1. Direct ticker match (highest weight)
/// 2. Category alignment
/// 3. Entity/keyword overlap with market title
///
/// Final score = conviction * urgency_multiplier * time_decay * match_strength
pub struct OsintScorer {
    signal_dir: PathBuf,
    /// Maximum age in seconds before a signal starts decaying
    decay_threshold_secs: i64,
    /// How aggressively signals decay after the threshold (higher = faster decay)
    decay_rate: f64,
}

impl OsintScorer {
    pub fn new(signal_dir: impl Into<PathBuf>) -> Self {
        Self {
            signal_dir: signal_dir.into(),
            decay_threshold_secs: 3600, // 1 hour
            decay_rate: 2.0,
        }
    }

    pub fn with_decay(mut self, threshold_secs: i64, rate: f64) -> Self {
        self.decay_threshold_secs = threshold_secs;
        self.decay_rate = rate;
        self
    }

    /// Load all signal files from the signal directory.
    fn load_signals(&self) -> Vec<OsintSignal> {
        let dir = &self.signal_dir;
        if !dir.exists() {
            debug!("signal directory does not exist: {}", dir.display());
            return Vec::new();
        }

        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                error!("failed to read signal directory {}: {}", dir.display(), e);
                return Vec::new();
            }
        };

        let mut signals = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            match fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<OsintSignal>(&content) {
                    Ok(signal) => signals.push(signal),
                    Err(e) => warn!("failed to parse signal {}: {}", path.display(), e),
                },
                Err(e) => warn!("failed to read signal {}: {}", path.display(), e),
            }
        }

        debug!("loaded {} signals from {}", signals.len(), dir.display());
        signals
    }

    /// Calculate time decay for a signal based on its age.
    /// Returns 1.0 for signals within the threshold, decaying toward 0 after.
    fn time_decay(&self, signal_time: DateTime<Utc>, now: DateTime<Utc>) -> f64 {
        let age_secs = (now - signal_time).num_seconds();
        if age_secs <= self.decay_threshold_secs {
            return 1.0;
        }

        let overtime_hours =
            (age_secs - self.decay_threshold_secs) as f64 / 3600.0;
        (1.0 / (1.0 + overtime_hours * self.decay_rate)).max(0.0)
    }

    /// Score how well a signal matches a given market candidate.
    /// Returns a match strength between 0.0 and 1.0.
    fn match_strength(signal: &OsintSignal, candidate: &MarketCandidate) -> f64 {
        let mut strength = 0.0;

        // direct ticker match is the strongest signal
        let ticker_lower = candidate.ticker.to_lowercase();
        for t in &signal.relevant_tickers {
            if ticker_lower.contains(&t.to_lowercase())
                || t.to_lowercase().contains(&ticker_lower)
            {
                strength = 1.0;
                return strength;
            }
        }

        // category alignment (partial match)
        let category_match = match signal.category {
            Category::Geopolitics | Category::Military => {
                let geo_keywords = [
                    "war", "conflict", "peace", "ceasefire", "military",
                    "sanctions", "nato", "nuclear",
                ];
                let title_lower = candidate.title.to_lowercase();
                geo_keywords.iter().any(|kw| title_lower.contains(kw))
            }
            Category::Economic => {
                let econ_keywords = [
                    "oil", "inflation", "gdp", "recession", "trade",
                    "tariff", "interest rate", "commodity", "price",
                ];
                let title_lower = candidate.title.to_lowercase();
                econ_keywords.iter().any(|kw| title_lower.contains(kw))
            }
            Category::Political => {
                let pol_keywords = [
                    "election", "vote", "president", "congress",
                    "legislation", "impeach", "poll",
                ];
                let title_lower = candidate.title.to_lowercase();
                pol_keywords.iter().any(|kw| title_lower.contains(kw))
            }
            Category::Climate => {
                let climate_keywords = [
                    "hurricane", "temperature", "weather", "wildfire",
                    "earthquake", "flood", "drought", "climate",
                ];
                let title_lower = candidate.title.to_lowercase();
                climate_keywords.iter().any(|kw| title_lower.contains(kw))
            }
        };

        if category_match {
            strength += 0.3;
        }

        // entity/keyword overlap with market title
        let title_lower = candidate.title.to_lowercase();
        let category_lower = candidate.category.to_lowercase();
        let mut entity_hits = 0;
        for entity in &signal.entities {
            let entity_clean = entity.replace('_', " ").to_lowercase();
            if title_lower.contains(&entity_clean)
                || category_lower.contains(&entity_clean)
            {
                entity_hits += 1;
            }
        }

        if !signal.entities.is_empty() {
            let entity_ratio = entity_hits as f64 / signal.entities.len() as f64;
            strength += entity_ratio * 0.5;
        }

        // theme overlap with title
        for theme in &signal.themes {
            let theme_words: Vec<&str> = theme.split('-').collect();
            if theme_words.iter().any(|w| title_lower.contains(w)) {
                strength += 0.1;
            }
        }

        strength.min(1.0)
    }

    /// Calculate the final OSINT score for a candidate given all loaded signals.
    fn calculate_score(
        &self,
        candidate: &MarketCandidate,
        signals: &[OsintSignal],
        now: DateTime<Utc>,
    ) -> (f64, f64) {
        let mut best_conviction = 0.0_f64;
        let mut best_urgency_score = 0.0_f64;

        for signal in signals {
            let match_str = Self::match_strength(signal, candidate);
            if match_str < 0.1 {
                continue;
            }

            let decay = self.time_decay(signal.timestamp, now);
            let urgency_mult = signal.urgency.multiplier();
            let score = signal.conviction * urgency_mult * decay * match_str;

            if score > best_conviction {
                best_conviction = score;
                best_urgency_score = urgency_mult;
            }
        }

        (best_conviction, best_urgency_score)
    }
}

impl Default for OsintScorer {
    fn default() -> Self {
        Self::new("data/osint_signals")
    }
}

#[async_trait]
impl Scorer for OsintScorer {
    fn name(&self) -> &'static str {
        "OsintScorer"
    }

    async fn score(
        &self,
        context: &TradingContext,
        candidates: &[MarketCandidate],
    ) -> Result<Vec<MarketCandidate>, String> {
        let signals = self.load_signals();

        if signals.is_empty() {
            debug!("no OSINT signals available — returning zero scores");
            return Ok(candidates
                .iter()
                .map(|c| {
                    let mut scored = MarketCandidate {
                        scores: c.scores.clone(),
                        ..Default::default()
                    };
                    scored.scores.insert("osint_conviction".to_string(), 0.0);
                    scored.scores.insert("osint_urgency".to_string(), 0.0);
                    scored
                })
                .collect());
        }

        let scored = candidates
            .iter()
            .map(|c| {
                let (conviction, urgency) =
                    self.calculate_score(c, &signals, context.timestamp);
                let mut scored = MarketCandidate {
                    scores: c.scores.clone(),
                    ..Default::default()
                };
                scored
                    .scores
                    .insert("osint_conviction".to_string(), conviction);
                scored
                    .scores
                    .insert("osint_urgency".to_string(), urgency);
                scored
            })
            .collect();

        Ok(scored)
    }

    fn update(&self, candidate: &mut MarketCandidate, scored: MarketCandidate) {
        for key in ["osint_conviction", "osint_urgency"] {
            if let Some(score) = scored.scores.get(key) {
                candidate.scores.insert(key.to_string(), *score);
            }
        }
    }
}
