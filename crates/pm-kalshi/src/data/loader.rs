//! CSV and SQLite data loader for backtesting

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use csv::ReaderBuilder;
use pm_core::{MarketData, MarketResult, PricePoint, Side, TradeData};
use pm_store::SqliteStore;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use tracing::info;

#[derive(Debug, Deserialize)]
struct CsvMarket {
    ticker: String,
    title: String,
    category: String,
    #[serde(with = "flexible_datetime")]
    open_time: DateTime<Utc>,
    #[serde(with = "flexible_datetime")]
    close_time: DateTime<Utc>,
    result: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CsvTrade {
    #[serde(with = "flexible_datetime")]
    timestamp: DateTime<Utc>,
    ticker: String,
    price: f64,
    volume: u64,
    taker_side: String,
}

mod flexible_datetime {
    use chrono::{DateTime, NaiveDateTime, Utc};
    use serde::{self, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        if let Ok(dt) = DateTime::parse_from_rfc3339(&s) {
            return Ok(dt.with_timezone(&Utc));
        }

        if let Ok(dt) = NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S") {
            return Ok(dt.and_utc());
        }

        if let Ok(ts) = s.parse::<i64>() {
            return DateTime::from_timestamp(ts, 0)
                .ok_or_else(|| serde::de::Error::custom("invalid timestamp"));
        }

        Err(serde::de::Error::custom(format!(
            "could not parse datetime: {}",
            s
        )))
    }
}

pub struct HistoricalData {
    pub markets: HashMap<String, MarketData>,
    pub trades: Vec<TradeData>,
    pub(crate) trade_index: HashMap<String, Vec<usize>>,
}

impl HistoricalData {
    pub fn load(data_dir: &Path) -> Result<Self> {
        let markets_path = data_dir.join("markets.csv");
        let trades_path = data_dir.join("trades.csv");

        let markets = load_markets(&markets_path)
            .with_context(|| format!("loading markets from {:?}", markets_path))?;

        let trades = load_trades(&trades_path)
            .with_context(|| format!("loading trades from {:?}", trades_path))?;

        let mut trade_index: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, trade) in trades.iter().enumerate() {
            trade_index.entry(trade.ticker.clone()).or_default().push(i);
        }

        Ok(Self {
            markets,
            trades,
            trade_index,
        })
    }

    /// Load historical data from SQLite, filtered by date range
    pub async fn load_sqlite(
        store: &SqliteStore,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Self> {
        let start_str = start.to_rfc3339();
        let end_str = end.to_rfc3339();

        info!("loading markets from sqlite");
        let market_rows = store
            .get_historical_markets_in_range(&start_str, &end_str)
            .await
            .context("querying historical markets")?;

        let mut markets = HashMap::new();
        for row in &market_rows {
            let open_time = parse_datetime_str(&row.open_time)?;
            let close_time = parse_datetime_str(&row.close_time)?;
            let result = row
                .result
                .as_deref()
                .and_then(|r| match r.to_lowercase().as_str() {
                    "yes" => Some(MarketResult::Yes),
                    "no" => Some(MarketResult::No),
                    "cancelled" | "canceled" => Some(MarketResult::Cancelled),
                    _ => None,
                });

            markets.insert(
                row.ticker.clone(),
                MarketData {
                    ticker: row.ticker.clone(),
                    title: row.title.clone(),
                    category: row.category.clone(),
                    open_time,
                    close_time,
                    result,
                },
            );
        }
        info!(markets = markets.len(), "markets loaded from sqlite");

        info!("loading trades from sqlite");
        let trade_rows = store
            .get_historical_trades_in_range(&start_str, &end_str)
            .await
            .context("querying historical trades")?;

        let mut trades = Vec::with_capacity(trade_rows.len());
        for row in &trade_rows {
            let timestamp = parse_datetime_str(&row.timestamp)?;
            let side = match row.taker_side.to_lowercase().as_str() {
                "yes" | "buy" => Side::Yes,
                "no" | "sell" => Side::No,
                _ => continue,
            };
            let price = Decimal::from_str(&row.price).unwrap_or(Decimal::ZERO);

            trades.push(TradeData {
                timestamp,
                ticker: row.ticker.clone(),
                price,
                volume: row.volume as u64,
                taker_side: side,
            });
        }
        // trades should already be sorted from the ORDER BY but just in case
        trades.sort_by_key(|t| t.timestamp);
        info!(trades = trades.len(), "trades loaded from sqlite");

        let mut trade_index: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, trade) in trades.iter().enumerate() {
            trade_index.entry(trade.ticker.clone()).or_default().push(i);
        }

        Ok(Self {
            markets,
            trades,
            trade_index,
        })
    }

    pub fn get_active_markets(&self, at: DateTime<Utc>) -> Vec<&MarketData> {
        self.markets
            .values()
            .filter(|m| at >= m.open_time && at <= m.close_time)
            .collect()
    }

    pub fn get_trades_for_market(
        &self,
        ticker: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Vec<&TradeData> {
        self.trade_index
            .get(ticker)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|&i| {
                        let trade = &self.trades[i];
                        if trade.timestamp >= from && trade.timestamp < to {
                            Some(trade)
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn get_current_price(&self, ticker: &str, at: DateTime<Utc>) -> Option<Decimal> {
        self.trade_index.get(ticker).and_then(|indices| {
            indices
                .iter()
                .filter_map(|&i| {
                    let trade = &self.trades[i];
                    if trade.timestamp <= at {
                        Some(trade)
                    } else {
                        None
                    }
                })
                .last()
                .map(|t| t.price)
        })
    }

    pub fn get_price_history(
        &self,
        ticker: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Vec<PricePoint> {
        self.get_trades_for_market(ticker, from, to)
            .into_iter()
            .map(|t| PricePoint {
                timestamp: t.timestamp,
                yes_price: t.price,
                volume: t.volume,
            })
            .collect()
    }

    pub fn get_volume_24h(&self, ticker: &str, at: DateTime<Utc>) -> u64 {
        let from = at - chrono::Duration::hours(24);
        self.get_trades_for_market(ticker, from, at)
            .iter()
            .map(|t| t.volume)
            .sum()
    }

    pub fn get_order_flow_24h(&self, ticker: &str, at: DateTime<Utc>) -> (u64, u64) {
        let from = at - chrono::Duration::hours(24);
        let trades = self.get_trades_for_market(ticker, from, at);
        let buy_vol: u64 = trades
            .iter()
            .filter(|t| t.taker_side == Side::Yes)
            .map(|t| t.volume)
            .sum();
        let sell_vol: u64 = trades
            .iter()
            .filter(|t| t.taker_side == Side::No)
            .map(|t| t.volume)
            .sum();
        (buy_vol, sell_vol)
    }

    pub fn get_resolutions(&self, at: DateTime<Utc>) -> Vec<(&MarketData, MarketResult)> {
        self.markets
            .values()
            .filter_map(|m| {
                if m.close_time <= at {
                    m.result.map(|r| (m, r))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn get_resolution_at(&self, ticker: &str, at: DateTime<Utc>) -> Option<MarketResult> {
        self.markets
            .get(ticker)
            .and_then(|m| if m.close_time <= at { m.result } else { None })
    }
}

fn load_markets(path: &Path) -> Result<HashMap<String, MarketData>> {
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_path(path)?;

    let mut markets = HashMap::new();

    for result in reader.deserialize() {
        let record: CsvMarket = result?;
        let result = record
            .result
            .as_ref()
            .and_then(|r| match r.to_lowercase().as_str() {
                "yes" => Some(MarketResult::Yes),
                "no" => Some(MarketResult::No),
                "cancelled" | "canceled" => Some(MarketResult::Cancelled),
                "" => None,
                _ => None,
            });

        markets.insert(
            record.ticker.clone(),
            MarketData {
                ticker: record.ticker,
                title: record.title,
                category: record.category,
                open_time: record.open_time,
                close_time: record.close_time,
                result,
            },
        );
    }

    Ok(markets)
}

fn load_trades(path: &Path) -> Result<Vec<TradeData>> {
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_path(path)?;

    let mut trades = Vec::new();

    for result in reader.deserialize() {
        let record: CsvTrade = result?;
        let side = match record.taker_side.to_lowercase().as_str() {
            "yes" | "buy" => Side::Yes,
            "no" | "sell" => Side::No,
            _ => continue,
        };

        trades.push(TradeData {
            timestamp: record.timestamp,
            ticker: record.ticker,
            price: Decimal::try_from(record.price / 100.0).unwrap_or(Decimal::ZERO),
            volume: record.volume,
            taker_side: side,
        });
    }

    trades.sort_by_key(|t| t.timestamp);

    Ok(trades)
}

/// Parse a datetime string that could be RFC3339, "YYYY-MM-DD HH:MM:SS", or unix timestamp
fn parse_datetime_str(s: &str) -> Result<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Ok(dt.and_utc());
    }
    if let Ok(ts) = s.parse::<i64>() {
        return DateTime::from_timestamp(ts, 0)
            .ok_or_else(|| anyhow::anyhow!("invalid timestamp: {}", s));
    }
    Err(anyhow::anyhow!("could not parse datetime: {}", s))
}

/// Ingest CSV data into SQLite for faster backtesting
pub async fn ingest_csv_to_sqlite(data_dir: &Path, store: &SqliteStore) -> Result<()> {
    let markets_path = data_dir.join("markets.csv");
    let trades_path = data_dir.join("trades.csv");

    // Ingest markets
    info!(path = %markets_path.display(), "ingesting markets CSV");
    store
        .clear_historical_markets()
        .await
        .context("clearing markets")?;

    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_path(&markets_path)
        .with_context(|| format!("opening {:?}", markets_path))?;

    let mut market_count: u64 = 0;
    for result in reader.deserialize() {
        let record: CsvMarket = result?;
        let result_str = record
            .result
            .as_ref()
            .and_then(|r| match r.to_lowercase().as_str() {
                "yes" | "no" | "cancelled" | "canceled" => Some(r.to_lowercase()),
                _ => None,
            });

        store
            .upsert_historical_market(
                &record.ticker,
                &record.title,
                &record.category,
                &record.open_time.to_rfc3339(),
                &record.close_time.to_rfc3339(),
                result_str.as_deref(),
            )
            .await?;

        market_count += 1;
        if market_count % 10_000 == 0 {
            info!(markets = market_count, "markets ingested");
        }
    }
    info!(total = market_count, "markets ingest complete");

    // Ingest trades in batches
    info!(path = %trades_path.display(), "ingesting trades CSV");
    store
        .clear_historical_trades()
        .await
        .context("clearing trades")?;

    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_path(&trades_path)
        .with_context(|| format!("opening {:?}", trades_path))?;

    let mut batch: Vec<(String, String, String, i64, String)> = Vec::with_capacity(10_000);
    let mut trade_count: u64 = 0;

    for result in reader.deserialize() {
        let record: CsvTrade = result?;
        let side = match record.taker_side.to_lowercase().as_str() {
            "yes" | "buy" => "yes",
            "no" | "sell" => "no",
            _ => continue,
        };

        let price = Decimal::try_from(record.price / 100.0)
            .unwrap_or(Decimal::ZERO)
            .to_string();

        batch.push((
            record.timestamp.to_rfc3339(),
            record.ticker,
            price,
            record.volume as i64,
            side.to_string(),
        ));

        if batch.len() >= 10_000 {
            trade_count += batch.len() as u64;
            store.insert_historical_trades_batch(&batch).await?;
            batch.clear();
            if trade_count % 100_000 == 0 {
                info!(trades = trade_count, "trades ingested");
            }
        }
    }

    if !batch.is_empty() {
        trade_count += batch.len() as u64;
        store.insert_historical_trades_batch(&batch).await?;
    }
    info!(total = trade_count, "trades ingest complete");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_data() -> TempDir {
        let dir = TempDir::new().unwrap();

        let markets_csv = r#"ticker,title,category,open_time,close_time,result
TEST-MKT-1,Test Market 1,politics,2024-01-01 00:00:00,2024-01-15 00:00:00,yes
TEST-MKT-2,Test Market 2,economics,2024-01-01 00:00:00,2024-01-20 00:00:00,no
"#;
        let mut f = std::fs::File::create(dir.path().join("markets.csv")).unwrap();
        f.write_all(markets_csv.as_bytes()).unwrap();

        let trades_csv = r#"timestamp,ticker,price,volume,taker_side
2024-01-05 12:00:00,TEST-MKT-1,55,100,yes
2024-01-05 13:00:00,TEST-MKT-1,57,50,yes
2024-01-06 10:00:00,TEST-MKT-2,45,200,no
"#;
        let mut f = std::fs::File::create(dir.path().join("trades.csv")).unwrap();
        f.write_all(trades_csv.as_bytes()).unwrap();

        dir
    }

    #[test]
    fn test_load_historical_data() {
        let dir = create_test_data();
        let data = HistoricalData::load(dir.path()).unwrap();

        assert_eq!(data.markets.len(), 2);
        assert_eq!(data.trades.len(), 3);
    }
}
