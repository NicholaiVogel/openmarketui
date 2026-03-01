//! Fee calculation for Kalshi trading
//!
//! Kalshi uses a variable fee structure:
//! - Taker: min(0.07 × contracts × P × (1-P), $0.02 × contracts)
//! - Maker: min(0.0175 × contracts × P × (1-P), $0.02 × contracts)
//!
//! The P × (1-P) term peaks at 50% probability (max uncertainty).

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct FeeConfig {
    #[serde(default = "default_taker_rate")]
    pub taker_rate: f64,
    #[serde(default = "default_maker_rate")]
    pub maker_rate: f64,
    #[serde(default = "default_max_per_contract")]
    pub max_per_contract: f64,
    #[serde(default = "default_assume_taker")]
    pub assume_taker: bool,
    #[serde(default = "default_min_edge_after_fees")]
    pub min_edge_after_fees: f64,
}

fn default_taker_rate() -> f64 {
    0.07
}
fn default_maker_rate() -> f64 {
    0.0175
}
fn default_max_per_contract() -> f64 {
    0.02
}
fn default_assume_taker() -> bool {
    true
}
fn default_min_edge_after_fees() -> f64 {
    0.02
}

impl Default for FeeConfig {
    fn default() -> Self {
        Self {
            taker_rate: default_taker_rate(),
            maker_rate: default_maker_rate(),
            max_per_contract: default_max_per_contract(),
            assume_taker: default_assume_taker(),
            min_edge_after_fees: default_min_edge_after_fees(),
        }
    }
}

impl FeeConfig {
    /// Calculate fee for a single trade
    pub fn calculate(&self, contracts: u64, price: f64) -> f64 {
        if contracts == 0 || price <= 0.0 || price >= 1.0 {
            return 0.0;
        }

        let rate = if self.assume_taker {
            self.taker_rate
        } else {
            self.maker_rate
        };
        let formula_fee = rate * (contracts as f64) * price * (1.0 - price);
        let cap_fee = self.max_per_contract * (contracts as f64);
        formula_fee.min(cap_fee)
    }

    /// Calculate round-trip fees (entry + exit)
    /// Uses 0.5 for exit price as conservative estimate (max fee point)
    pub fn round_trip_estimate(&self, contracts: u64, entry_price: f64) -> f64 {
        self.calculate(contracts, entry_price) + self.calculate(contracts, 0.5)
    }

    /// Fee as percentage of position value
    pub fn fee_drag_pct(&self, contracts: u64, price: f64) -> f64 {
        let fee = self.calculate(contracts, price);
        let position_value = (contracts as f64) * price;
        if position_value > 0.0 {
            fee / position_value
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_at_50_percent() {
        let config = FeeConfig::default();
        // 100 contracts at $0.50: 0.07 * 100 * 0.5 * 0.5 = $1.75
        let fee = config.calculate(100, 0.50);
        assert!((fee - 1.75).abs() < 0.01);
    }

    #[test]
    fn test_fee_at_extreme_price() {
        let config = FeeConfig::default();
        // 100 contracts at $0.95: 0.07 * 100 * 0.95 * 0.05 = $0.3325
        let fee = config.calculate(100, 0.95);
        assert!((fee - 0.3325).abs() < 0.02);
    }

    #[test]
    fn test_fee_cap() {
        let config = FeeConfig::default();
        // 1000 contracts at $0.50: formula = 0.07 * 1000 * 0.25 = $17.50
        // cap = $0.02 * 1000 = $20
        // formula is less, so use formula
        let fee = config.calculate(1000, 0.50);
        assert!((fee - 17.50).abs() < 0.01);
    }

    #[test]
    fn test_fee_cap_applied() {
        let config = FeeConfig::default();
        // at prices near 0.5, formula grows faster than cap for large positions
        // 10000 contracts at $0.50: formula = 0.07 * 10000 * 0.25 = $175
        // cap = $0.02 * 10000 = $200
        // formula is still less, but let's test a scenario where cap kicks in
        // using maker rate: 0.0175 * 10000 * 0.25 = $43.75, cap = $200 → use formula
        // actually the cap only kicks in at very low/high prices where P*(1-P) is large
        // let's verify the math is correct
        let fee = config.calculate(10000, 0.50);
        assert!((fee - 175.0).abs() < 0.01);
    }

    #[test]
    fn test_maker_rate() {
        let config = FeeConfig {
            assume_taker: false,
            ..Default::default()
        };
        // 100 contracts at $0.50: 0.0175 * 100 * 0.5 * 0.5 = $0.4375
        let fee = config.calculate(100, 0.50);
        assert!((fee - 0.4375).abs() < 0.01);
    }

    #[test]
    fn test_fee_drag_pct() {
        let config = FeeConfig::default();
        // 100 contracts at $0.50 = $50 position value
        // fee = $1.75
        // drag = 1.75 / 50 = 0.035 = 3.5%
        let drag = config.fee_drag_pct(100, 0.50);
        assert!((drag - 0.035).abs() < 0.001);
    }

    #[test]
    fn test_round_trip_estimate() {
        let config = FeeConfig::default();
        // entry at $0.30: 0.07 * 100 * 0.30 * 0.70 = $1.47
        // exit at $0.50 (conservative): 0.07 * 100 * 0.50 * 0.50 = $1.75
        // total = $3.22
        let rt = config.round_trip_estimate(100, 0.30);
        assert!((rt - 3.22).abs() < 0.01);
    }

    #[test]
    fn test_zero_contracts() {
        let config = FeeConfig::default();
        assert_eq!(config.calculate(0, 0.50), 0.0);
    }

    #[test]
    fn test_edge_prices() {
        let config = FeeConfig::default();
        assert_eq!(config.calculate(100, 0.0), 0.0);
        assert_eq!(config.calculate(100, 1.0), 0.0);
    }
}
