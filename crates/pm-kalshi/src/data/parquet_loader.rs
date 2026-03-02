//! Parquet data loader for Becker's prediction-market-analysis dataset
//!
//! Reads sharded parquet files from:
//!   {data_dir}/kalshi/markets/*.parquet
//!   {data_dir}/kalshi/trades/*.parquet
//!
//! Schema (markets):
//!   ticker: string, event_ticker: string, title: string, status: string,
//!   result: string, volume: int64, volume_24h: int64,
//!   open_time: timestamp[ns,UTC], close_time: timestamp[ns,UTC],
//!   yes_bid: int64, yes_ask: int64, last_price: int64
//!
//! Schema (trades):
//!   trade_id: string, ticker: string, count: int64,
//!   yes_price: int64, no_price: int64, taker_side: string,
//!   created_time: timestamp[ns,UTC]

use super::HistoricalData;
use anyhow::{Context, Result};
use arrow::array::{Array, AsArray, RecordBatch};
use arrow::datatypes::DataType;
use chrono::{DateTime, TimeZone, Utc};
use pm_core::{MarketData, MarketResult, Side, TradeData};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

/// Load Kalshi historical data from Becker's parquet dataset.
///
/// `data_dir` should point to the root `data/` directory containing
/// `kalshi/markets/` and `kalshi/trades/` subdirectories.
///
/// If `time_range` is provided, only markets overlapping that range
/// and trades within it are loaded — essential for the full dataset
/// which is multiple GB.
pub fn load_parquet(
    data_dir: &Path,
    time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
) -> Result<HistoricalData> {
    let markets_dir = data_dir.join("kalshi").join("markets");
    let trades_dir = data_dir.join("kalshi").join("trades");

    anyhow::ensure!(markets_dir.is_dir(), "markets dir not found: {}", markets_dir.display());
    anyhow::ensure!(trades_dir.is_dir(), "trades dir not found: {}", trades_dir.display());

    let markets = load_markets_parquet(&markets_dir, time_range)
        .context("loading parquet markets")?;
    info!(markets = markets.len(), "loaded markets from parquet");

    let trades = load_trades_parquet(&trades_dir, time_range, &markets)
        .context("loading parquet trades")?;
    info!(trades = trades.len(), "loaded trades from parquet");

    let mut trade_index: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, trade) in trades.iter().enumerate() {
        trade_index.entry(trade.ticker.clone()).or_default().push(i);
    }

    Ok(HistoricalData {
        markets,
        trades,
        trade_index,
    })
}

fn load_markets_parquet(
    dir: &Path,
    time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
) -> Result<HashMap<String, MarketData>> {
    let mut markets = HashMap::new();
    let mut files: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "parquet"))
        .map(|e| e.path())
        .collect();
    files.sort();

    info!(files = files.len(), "reading market parquet files");

    for path in &files {
        let file = std::fs::File::open(path)
            .with_context(|| format!("opening {}", path.display()))?;
        let reader = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)?
            .build()?;

        for batch_result in reader {
            let batch = batch_result?;
            extract_markets(&batch, &mut markets, time_range)?;
        }
    }

    Ok(markets)
}

fn extract_markets(
    batch: &RecordBatch,
    markets: &mut HashMap<String, MarketData>,
    time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
) -> Result<()> {
    let schema = batch.schema();
    let n = batch.num_rows();

    let ticker_col = batch.column(schema.index_of("ticker")?).as_string::<i32>();
    let title_col = batch.column(schema.index_of("title")?).as_string::<i32>();
    let status_col = batch.column(schema.index_of("status")?).as_string::<i32>();
    let result_col = batch.column(schema.index_of("result")?).as_string::<i32>();

    let open_time_idx = schema.index_of("open_time")?;
    let close_time_idx = schema.index_of("close_time")?;

    // event_ticker serves as a coarse category for Kalshi markets
    let event_ticker_col = batch.column(schema.index_of("event_ticker")?).as_string::<i32>();

    for i in 0..n {
        if ticker_col.is_null(i) || status_col.is_null(i) {
            continue;
        }

        let ticker = ticker_col.value(i);
        let status = status_col.value(i);

        // Only load finalized markets (we need results for backtesting)
        // plus open/active markets for time-range filtering
        if status != "finalized" && status != "closed" && status != "active" && status != "open" {
            continue;
        }

        let open_time = timestamp_from_column(batch, open_time_idx, i);
        let close_time = timestamp_from_column(batch, close_time_idx, i);

        let (open_time, close_time) = match (open_time, close_time) {
            (Some(o), Some(c)) => (o, c),
            _ => continue,
        };

        // Time range filter: skip markets that don't overlap
        if let Some((start, end)) = time_range {
            if close_time < start || open_time > end {
                continue;
            }
        }

        let result_str = if result_col.is_null(i) {
            ""
        } else {
            result_col.value(i)
        };

        let result = match result_str.to_lowercase().as_str() {
            "yes" => Some(MarketResult::Yes),
            "no" => Some(MarketResult::No),
            _ => None,
        };

        let title = if title_col.is_null(i) {
            String::new()
        } else {
            title_col.value(i).to_string()
        };

        // Use event_ticker prefix as category (e.g. "KXPOLITICS", "KXECON")
        let category = if event_ticker_col.is_null(i) {
            String::new()
        } else {
            categorize_event_ticker(event_ticker_col.value(i))
        };

        markets.insert(
            ticker.to_string(),
            MarketData {
                ticker: ticker.to_string(),
                title,
                category,
                open_time,
                close_time,
                result,
            },
        );
    }

    Ok(())
}

fn load_trades_parquet(
    dir: &Path,
    time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    markets: &HashMap<String, MarketData>,
) -> Result<Vec<TradeData>> {
    let mut files: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "parquet"))
        .map(|e| e.path())
        .collect();
    files.sort();

    info!(files = files.len(), "reading trade parquet files");

    let mut trades = Vec::new();
    let mut file_count = 0;

    for path in &files {
        let file = std::fs::File::open(path)
            .with_context(|| format!("opening {}", path.display()))?;
        let reader = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)?
            .build()?;

        for batch_result in reader {
            let batch = batch_result?;
            extract_trades(&batch, &mut trades, time_range, markets)?;
        }

        file_count += 1;
        if file_count % 500 == 0 {
            info!(files_read = file_count, trades_so_far = trades.len(), "loading trades...");
        }
    }

    trades.sort_by_key(|t| t.timestamp);
    Ok(trades)
}

fn extract_trades(
    batch: &RecordBatch,
    trades: &mut Vec<TradeData>,
    time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    markets: &HashMap<String, MarketData>,
) -> Result<()> {
    let schema = batch.schema();
    let n = batch.num_rows();

    let ticker_col = batch.column(schema.index_of("ticker")?).as_string::<i32>();
    let count_col = batch.column(schema.index_of("count")?).as_primitive::<arrow::datatypes::Int64Type>();
    let yes_price_col = batch.column(schema.index_of("yes_price")?).as_primitive::<arrow::datatypes::Int64Type>();
    let taker_side_col = batch.column(schema.index_of("taker_side")?).as_string::<i32>();
    let created_time_idx = schema.index_of("created_time")?;

    for i in 0..n {
        if ticker_col.is_null(i) || taker_side_col.is_null(i) {
            continue;
        }

        let ticker = ticker_col.value(i);

        // Only load trades for markets we have
        if !markets.contains_key(ticker) {
            continue;
        }

        let timestamp = match timestamp_from_column(batch, created_time_idx, i) {
            Some(ts) => ts,
            None => continue,
        };

        if let Some((start, end)) = time_range {
            if timestamp < start || timestamp > end {
                continue;
            }
        }

        let side = match taker_side_col.value(i).to_lowercase().as_str() {
            "yes" => Side::Yes,
            "no" => Side::No,
            _ => continue,
        };

        // Becker's prices are in cents (1-99), convert to decimal (0.01-0.99)
        let yes_price_cents = yes_price_col.value(i);
        let price = Decimal::new(yes_price_cents, 2);

        let volume = count_col.value(i) as u64;

        trades.push(TradeData {
            timestamp,
            ticker: ticker.to_string(),
            price,
            volume,
            taker_side: side,
        });
    }

    Ok(())
}

/// Extract a UTC timestamp from an arrow column.
///
/// Handles both TimestampNanosecond and TimestampMicrosecond types,
/// with or without timezone info.
fn timestamp_from_column(batch: &RecordBatch, col_idx: usize, row: usize) -> Option<DateTime<Utc>> {
    let col = batch.column(col_idx);
    if col.is_null(row) {
        return None;
    }

    match col.data_type() {
        DataType::Timestamp(arrow::datatypes::TimeUnit::Nanosecond, _) => {
            let arr = col.as_primitive::<arrow::datatypes::TimestampNanosecondType>();
            let nanos = arr.value(row);
            let secs = nanos / 1_000_000_000;
            let nsec = (nanos % 1_000_000_000) as u32;
            Utc.timestamp_opt(secs, nsec).single()
        }
        DataType::Timestamp(arrow::datatypes::TimeUnit::Microsecond, _) => {
            let arr = col.as_primitive::<arrow::datatypes::TimestampMicrosecondType>();
            let micros = arr.value(row);
            let secs = micros / 1_000_000;
            let nsec = ((micros % 1_000_000) * 1_000) as u32;
            Utc.timestamp_opt(secs, nsec).single()
        }
        DataType::Timestamp(arrow::datatypes::TimeUnit::Millisecond, _) => {
            let arr = col.as_primitive::<arrow::datatypes::TimestampMillisecondType>();
            let millis = arr.value(row);
            let secs = millis / 1_000;
            let nsec = ((millis % 1_000) * 1_000_000) as u32;
            Utc.timestamp_opt(secs, nsec).single()
        }
        _ => None,
    }
}

/// Map Kalshi event_ticker prefixes to human-readable categories.
///
/// Kalshi event tickers follow patterns like:
///   KXPOLITICS-*, KXECON-*, KXSCIENCE-*, KXMVSPORTS-*, etc.
fn categorize_event_ticker(event_ticker: &str) -> String {
    let upper = event_ticker.to_uppercase();
    if upper.contains("POLITIC") || upper.contains("PRES") || upper.contains("SCOTUS")
        || upper.contains("CONGRESS") || upper.contains("SENATE") || upper.contains("HOUSE")
        || upper.contains("ELECT")
    {
        "politics".to_string()
    } else if upper.contains("ECON") || upper.contains("GDP") || upper.contains("INFLATION")
        || upper.contains("FED") || upper.contains("CPI") || upper.contains("JOBS")
        || upper.contains("RATE") || upper.contains("RECESSION")
    {
        "economics".to_string()
    } else if upper.contains("SPORT") || upper.contains("NBA") || upper.contains("NFL")
        || upper.contains("MLB") || upper.contains("NHL") || upper.contains("FIFA")
        || upper.contains("ESPORT")
    {
        "sports".to_string()
    } else if upper.contains("WEATHER") || upper.contains("CLIMATE") || upper.contains("TEMP")
        || upper.contains("HURRICANE")
    {
        "climate".to_string()
    } else if upper.contains("CRYPTO") || upper.contains("BTC") || upper.contains("ETH")
        || upper.contains("BITCOIN")
    {
        "crypto".to_string()
    } else if upper.contains("WAR") || upper.contains("CONFLICT") || upper.contains("IRAN")
        || upper.contains("RUSSIA") || upper.contains("UKRAINE") || upper.contains("NATO")
        || upper.contains("CHINA") || upper.contains("TAIWAN")
    {
        "geopolitics".to_string()
    } else if upper.contains("SCIENCE") || upper.contains("TECH") || upper.contains("AI")
        || upper.contains("SPACE")
    {
        "science".to_string()
    } else {
        "general".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_event_ticker() {
        assert_eq!(categorize_event_ticker("KXPOLITICS-2024-DJT"), "politics");
        assert_eq!(categorize_event_ticker("KXECON-GDP-Q4"), "economics");
        assert_eq!(categorize_event_ticker("KXMVSPORTS-NBA"), "sports");
        assert_eq!(categorize_event_ticker("KXWEATHER-HURRICANE"), "climate");
        assert_eq!(categorize_event_ticker("KXCRYPTO-BTC-100K"), "crypto");
        assert_eq!(categorize_event_ticker("IRAN-WAR-2026"), "geopolitics");
        assert_eq!(categorize_event_ticker("SOME-RANDOM-THING"), "general");
    }

    #[test]
    fn test_cents_to_decimal() {
        let price = Decimal::new(65, 2);
        assert_eq!(price.to_string(), "0.65");
    }

    /// Integration test — only runs if becker's dataset is present
    #[test]
    fn test_load_real_parquet() {
        let data_dir = Path::new("/mnt/work/prediction-market-analysis/data");
        if !data_dir.exists() {
            eprintln!("skipping: becker dataset not found at {}", data_dir.display());
            return;
        }

        let start = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2024, 6, 7, 0, 0, 0).unwrap();

        let data = load_parquet(data_dir, Some((start, end)))
            .expect("failed to load parquet");

        assert!(!data.markets.is_empty(), "should have loaded some markets");
        assert!(!data.trades.is_empty(), "should have loaded some trades");

        // Verify price conversion — all prices should be between 0 and 1
        for trade in &data.trades {
            assert!(trade.price >= Decimal::ZERO, "price should be >= 0: {}", trade.price);
            assert!(trade.price <= Decimal::ONE, "price should be <= 1: {}", trade.price);
        }

        // Verify trade timestamps are in range
        for trade in &data.trades {
            assert!(trade.timestamp >= start, "trade before start: {}", trade.timestamp);
            assert!(trade.timestamp <= end, "trade after end: {}", trade.timestamp);
        }

        // Verify HistoricalData methods work
        let mid = Utc.with_ymd_and_hms(2024, 6, 3, 12, 0, 0).unwrap();
        let active = data.get_active_markets(mid);
        assert!(!active.is_empty(), "should have active markets at midpoint");

        eprintln!("loaded {} markets, {} trades, {} active at midpoint",
            data.markets.len(), data.trades.len(), active.len());
    }
}
