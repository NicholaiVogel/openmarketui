//! Data fetcher for Kalshi historical market + trade data
//!
//! Ports the functionality from tools/fetch_kalshi_data_v2.py to Rust,
//! allowing data fetching to be triggered from the web UI.

use chrono::NaiveDate;
use pm_store::SqliteStore;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tracing::{info, warn};

const BASE_URL: &str = "https://api.elections.kalshi.com/trade-api/v2";
const RATE_LIMIT_DELAY_MS: u64 = 300;
const MAX_RETRIES: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchStatus {
    Idle,
    Fetching,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct FetchProgress {
    pub status: FetchStatus,
    pub phase: Option<String>,
    pub current_day: Option<String>,
    pub days_complete: usize,
    pub days_total: usize,
    pub trades_fetched: usize,
    pub markets_fetched: usize,
    pub markets_done: bool,
    pub error: Option<String>,
}

impl Default for FetchProgress {
    fn default() -> Self {
        Self {
            status: FetchStatus::Idle,
            phase: None,
            current_day: None,
            days_complete: 0,
            days_total: 0,
            trades_fetched: 0,
            markets_fetched: 0,
            markets_done: false,
            error: None,
        }
    }
}

#[derive(Debug)]
pub struct FetchState {
    pub status: FetchStatus,
    pub phase: Option<String>,
    pub current_day: Option<String>,
    pub days_complete: AtomicUsize,
    pub days_total: AtomicUsize,
    pub trades_fetched: AtomicUsize,
    pub markets_fetched: AtomicUsize,
    pub markets_done: AtomicBool,
    pub error: Option<String>,
    pub cancel_requested: AtomicBool,
}

impl FetchState {
    pub fn new() -> Self {
        Self {
            status: FetchStatus::Idle,
            phase: None,
            current_day: None,
            days_complete: AtomicUsize::new(0),
            days_total: AtomicUsize::new(0),
            trades_fetched: AtomicUsize::new(0),
            markets_fetched: AtomicUsize::new(0),
            markets_done: AtomicBool::new(false),
            error: None,
            cancel_requested: AtomicBool::new(false),
        }
    }

    pub fn to_progress(&self) -> FetchProgress {
        FetchProgress {
            status: self.status,
            phase: self.phase.clone(),
            current_day: self.current_day.clone(),
            days_complete: self.days_complete.load(Ordering::Relaxed),
            days_total: self.days_total.load(Ordering::Relaxed),
            trades_fetched: self.trades_fetched.load(Ordering::Relaxed),
            markets_fetched: self.markets_fetched.load(Ordering::Relaxed),
            markets_done: self.markets_done.load(Ordering::Relaxed),
            error: self.error.clone(),
        }
    }
}

impl Default for FetchState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct TradesResponse {
    trades: Vec<Trade>,
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MarketsResponse {
    markets: Vec<Market>,
}

#[derive(Debug, Deserialize)]
struct MarketResponse {
    market: Option<Market>,
}

#[derive(Debug, Deserialize)]
struct Trade {
    created_time: Option<String>,
    ts: Option<String>,
    ticker: Option<String>,
    market_ticker: Option<String>,
    yes_price: Option<serde_json::Value>,
    price: Option<serde_json::Value>,
    count: Option<u64>,
    volume: Option<u64>,
    taker_side: Option<String>,
    is_taker_side_yes: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct Market {
    ticker: Option<String>,
    title: Option<String>,
    category: Option<String>,
    open_time: Option<String>,
    close_time: Option<String>,
    expiration_time: Option<String>,
    result: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FetchStateFile {
    #[serde(default)]
    completed_days: Vec<String>,
    #[serde(default)]
    current_day: Option<String>,
    #[serde(default)]
    current_day_cursor: Option<String>,
    #[serde(default)]
    current_day_count: usize,
    #[serde(default)]
    total_trades: usize,
    #[serde(default)]
    markets_cursor: Option<String>,
    #[serde(default)]
    markets_count: usize,
    #[serde(default)]
    markets_done: bool,
}

struct DayFetchResult {
    count: usize,
    cancelled: bool,
}

impl Default for FetchStateFile {
    fn default() -> Self {
        Self {
            completed_days: Vec::new(),
            current_day: None,
            current_day_cursor: None,
            current_day_count: 0,
            total_trades: 0,
            markets_cursor: None,
            markets_count: 0,
            markets_done: false,
        }
    }
}

pub struct DataFetcher {
    client: reqwest::Client,
    output_dir: PathBuf,
    store: Arc<SqliteStore>,
}

impl DataFetcher {
    pub fn new(output_dir: PathBuf, store: Arc<SqliteStore>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            output_dir,
            store,
        }
    }

    pub async fn fetch_range(
        &self,
        start: NaiveDate,
        end: NaiveDate,
        trades_per_day: usize,
        fetch_markets: bool,
        fetch_trades: bool,
        state: Arc<tokio::sync::RwLock<FetchState>>,
    ) -> anyhow::Result<()> {
        if !fetch_markets && !fetch_trades {
            let mut guard = state.write().await;
            guard.status = FetchStatus::Complete;
            guard.phase = Some("complete".to_string());
            guard.current_day = None;
            return Ok(());
        }

        tokio::fs::create_dir_all(&self.output_dir).await?;

        let state_path = self.output_dir.join("fetch_state_v2.json");
        let mut file_state = self.load_state(&state_path).await?;

        let db_trades = self.store.count_historical_trades().await? as usize;
        let db_markets = self.store.count_historical_markets().await? as usize;

        if fetch_trades
            && db_trades == 0
            && (!file_state.completed_days.is_empty()
                || file_state.current_day.is_some()
                || file_state.current_day_cursor.is_some()
                || file_state.current_day_count > 0
                || file_state.total_trades > 0)
        {
            warn!("trade state existed but DB has 0 trades; resetting trade resume state");
            file_state.completed_days.clear();
            file_state.current_day = None;
            file_state.current_day_cursor = None;
            file_state.current_day_count = 0;
            file_state.total_trades = 0;
            self.save_state(&state_path, &file_state).await?;
        } else if fetch_trades && file_state.total_trades > db_trades {
            file_state.total_trades = db_trades;
            self.save_state(&state_path, &file_state).await?;
        }

        if fetch_markets
            && db_markets == 0
            && (file_state.markets_count > 0
                || file_state.markets_cursor.is_some()
                || file_state.markets_done)
        {
            warn!("market state existed but DB has 0 markets; resetting market resume state");
            file_state.markets_cursor = None;
            file_state.markets_count = 0;
            file_state.markets_done = false;
            self.save_state(&state_path, &file_state).await?;
        }

        let all_days = if fetch_trades {
            Self::generate_date_range(start, end)
        } else {
            Vec::new()
        };
        let completed: HashSet<String> = if fetch_trades {
            file_state.completed_days.iter().cloned().collect()
        } else {
            HashSet::new()
        };
        let remaining_days: Vec<String> = if fetch_trades {
            all_days
                .iter()
                .filter(|d| !completed.contains(*d))
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        let completed_in_range = if fetch_trades {
            all_days.iter().filter(|d| completed.contains(*d)).count()
        } else {
            0
        };

        {
            let mut guard = state.write().await;
            guard.status = FetchStatus::Fetching;
            guard.phase = Some(if fetch_trades {
                "fetching_trades".to_string()
            } else {
                "fetching_markets".to_string()
            });
            guard.days_total.store(all_days.len(), Ordering::Relaxed);
            guard
                .days_complete
                .store(completed_in_range, Ordering::Relaxed);
            guard
                .trades_fetched
                .store(file_state.total_trades, Ordering::Relaxed);
            guard
                .markets_fetched
                .store(file_state.markets_count, Ordering::Relaxed);
            guard
                .markets_done
                .store(file_state.markets_done, Ordering::Relaxed);
            guard.current_day = None;
        }

        info!(
            start = %start,
            end = %end,
            fetch_markets = fetch_markets,
            fetch_trades = fetch_trades,
            total_days = all_days.len(),
            completed = completed_in_range,
            remaining = remaining_days.len(),
            "starting data fetch"
        );

        if fetch_trades {
            for day in remaining_days {
                {
                    let guard = state.read().await;
                    if guard.cancel_requested.load(Ordering::Relaxed) {
                        let mut guard = state.write().await;
                        guard.status = FetchStatus::Cancelled;
                        guard.phase = Some("cancelled".to_string());
                        info!("data fetch cancelled by user");
                        return Ok(());
                    }
                }

                {
                    let mut guard = state.write().await;
                    guard.current_day = Some(day.clone());
                }

                if file_state.current_day.as_ref() == Some(&day) {
                    info!(day = %day, count = file_state.current_day_count, "resuming day");
                } else {
                    file_state.current_day = Some(day.clone());
                    file_state.current_day_cursor = None;
                    file_state.current_day_count = 0;
                    self.save_state(&state_path, &file_state).await?;
                    info!(day = %day, "fetching day");
                }

                let day_result = self
                    .fetch_day_trades(&day, trades_per_day, &state_path, &mut file_state, &state)
                    .await?;

                if day_result.cancelled {
                    let mut guard = state.write().await;
                    guard.status = FetchStatus::Cancelled;
                    guard.phase = Some("cancelled".to_string());
                    info!(day = %day, count = day_result.count, "data fetch cancelled during day");
                    return Ok(());
                }

                file_state.completed_days.push(day.clone());
                file_state.current_day = None;
                file_state.current_day_cursor = None;
                file_state.current_day_count = 0;
                self.save_state(&state_path, &file_state).await?;

                {
                    let guard = state.read().await;
                    guard.days_complete.fetch_add(1, Ordering::Relaxed);
                }

                info!(day = %day, count = day_result.count, "day complete");
            }
        }

        if fetch_markets {
            {
                let guard = state.read().await;
                if guard.cancel_requested.load(Ordering::Relaxed) {
                    let mut guard = state.write().await;
                    guard.status = FetchStatus::Cancelled;
                    guard.phase = Some("cancelled".to_string());
                    info!("data fetch cancelled by user before market phase");
                    return Ok(());
                }
            }

            {
                let mut guard = state.write().await;
                guard.phase = Some("fetching_markets".to_string());
                guard.current_day = Some("markets".to_string());
                guard.markets_done.store(false, Ordering::Relaxed);
            }

            let fetched_count = self
                .fetch_markets_for_traded_tickers(start, end, &state)
                .await?;

            {
                let guard = state.read().await;
                if guard.cancel_requested.load(Ordering::Relaxed) {
                    let mut guard = state.write().await;
                    guard.status = FetchStatus::Cancelled;
                    guard.phase = Some("cancelled".to_string());
                    guard.current_day = None;
                    info!("data fetch cancelled during market phase");
                    return Ok(());
                }
            }

            file_state.markets_count = self.store.count_historical_markets().await? as usize;
            file_state.markets_done = true;
            file_state.markets_cursor = None;
            self.save_state(&state_path, &file_state).await?;
            info!(
                fetched = fetched_count,
                total = file_state.markets_count,
                "market fetch complete"
            );
        }

        {
            let mut guard = state.write().await;
            guard.status = FetchStatus::Complete;
            guard.phase = Some("complete".to_string());
            guard.current_day = None;
            guard
                .markets_done
                .store(file_state.markets_done, Ordering::Relaxed);
        }

        info!(
            total_trades = file_state.total_trades,
            days_complete = file_state.completed_days.len(),
            "data fetch complete"
        );

        Ok(())
    }

    async fn fetch_markets_for_traded_tickers(
        &self,
        start: NaiveDate,
        end: NaiveDate,
        state: &Arc<tokio::sync::RwLock<FetchState>>,
    ) -> anyhow::Result<usize> {
        let start_date = start.format("%Y-%m-%d").to_string();
        let end_date = end.format("%Y-%m-%d").to_string();

        let traded_tickers = self
            .store
            .get_historical_trade_tickers_in_range(&start_date, &end_date)
            .await?;
        let existing_tickers: HashSet<String> = self
            .store
            .get_historical_market_tickers()
            .await?
            .into_iter()
            .collect();

        let mut pending_tickers: Vec<String> = traded_tickers
            .into_iter()
            .filter(|t| !t.is_empty() && !existing_tickers.contains(t))
            .collect();
        pending_tickers.sort();

        let total_pending = pending_tickers.len();
        if total_pending == 0 {
            let market_total = self.store.count_historical_markets().await? as usize;
            let mut guard = state.write().await;
            guard.markets_fetched.store(market_total, Ordering::Relaxed);
            guard.markets_done.store(true, Ordering::Relaxed);
            guard.current_day = Some("markets up-to-date".to_string());
            return Ok(0);
        }

        let mut batch: Vec<(String, String, String, String, String, Option<String>)> =
            Vec::with_capacity(100);
        let mut fetched = 0usize;

        for (idx, ticker) in pending_tickers.iter().enumerate() {
            {
                let guard = state.read().await;
                if guard.cancel_requested.load(Ordering::Relaxed) {
                    return Ok(fetched);
                }
            }

            {
                let mut guard = state.write().await;
                guard.current_day =
                    Some(format!("market {}/{} {}", idx + 1, total_pending, ticker));
            }

            if let Some(market) = self.fetch_market_by_ticker(ticker).await? {
                if let Some(row) = Self::market_to_upsert_tuple(&market) {
                    batch.push(row);
                }
            }

            if batch.len() >= 100 {
                self.store.upsert_historical_markets_batch(&batch).await?;
                fetched += batch.len();
                batch.clear();

                let market_total = self.store.count_historical_markets().await? as usize;
                let guard = state.read().await;
                guard.markets_fetched.store(market_total, Ordering::Relaxed);
            }

            tokio::time::sleep(std::time::Duration::from_millis(RATE_LIMIT_DELAY_MS)).await;
        }

        if !batch.is_empty() {
            self.store.upsert_historical_markets_batch(&batch).await?;
            fetched += batch.len();
        }

        let market_total = self.store.count_historical_markets().await? as usize;
        {
            let guard = state.read().await;
            guard.markets_fetched.store(market_total, Ordering::Relaxed);
            guard.markets_done.store(true, Ordering::Relaxed);
        }

        Ok(fetched)
    }

    async fn fetch_market_by_ticker(&self, ticker: &str) -> anyhow::Result<Option<Market>> {
        let url = format!("{}/markets/{}", BASE_URL, ticker);
        let response = self.fetch_with_retry(&url).await?;
        let payload: serde_json::Value = response.json().await?;

        if let Ok(wrapper) = serde_json::from_value::<MarketResponse>(payload.clone()) {
            if let Some(market) = wrapper.market {
                return Ok(Some(market));
            }
        }

        if let Ok(list) = serde_json::from_value::<MarketsResponse>(payload.clone()) {
            if let Some(market) = list.markets.into_iter().next() {
                return Ok(Some(market));
            }
        }

        if let Ok(market) = serde_json::from_value::<Market>(payload.clone()) {
            return Ok(Some(market));
        }

        warn!(ticker = %ticker, "unable to parse market detail response");
        Ok(None)
    }

    async fn fetch_day_trades(
        &self,
        day: &str,
        trades_per_day: usize,
        state_path: &PathBuf,
        file_state: &mut FetchStateFile,
        state: &Arc<tokio::sync::RwLock<FetchState>>,
    ) -> anyhow::Result<DayFetchResult> {
        let (min_ts, max_ts) = Self::date_to_timestamps(day)?;
        let mut cursor = file_state.current_day_cursor.clone();
        let mut count = file_state.current_day_count;

        while count < trades_per_day {
            {
                let guard = state.read().await;
                if guard.cancel_requested.load(Ordering::Relaxed) {
                    self.save_state(state_path, file_state).await?;
                    return Ok(DayFetchResult {
                        count,
                        cancelled: true,
                    });
                }
            }

            let mut url = format!(
                "{}/markets/trades?limit=1000&min_ts={}&max_ts={}",
                BASE_URL, min_ts, max_ts
            );
            if let Some(ref c) = cursor {
                url.push_str(&format!("&cursor={}", c));
            }

            let response = self.fetch_with_retry(&url).await?;
            let data: TradesResponse = response.json().await?;

            if data.trades.is_empty() {
                break;
            }

            self.insert_trades_batch(&data.trades).await?;

            count += data.trades.len();
            file_state.total_trades += data.trades.len();

            cursor = data.cursor.clone();
            file_state.current_day_cursor = cursor.clone();
            file_state.current_day_count = count;

            {
                let guard = state.read().await;
                guard
                    .trades_fetched
                    .store(file_state.total_trades, Ordering::Relaxed);
            }

            if count % 10000 == 0 {
                self.save_state(state_path, file_state).await?;
                info!(day = %day, count = count, "progress checkpoint");
            }

            if cursor.is_none() {
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(RATE_LIMIT_DELAY_MS)).await;
        }

        Ok(DayFetchResult {
            count,
            cancelled: false,
        })
    }

    async fn fetch_with_retry(&self, url: &str) -> anyhow::Result<reqwest::Response> {
        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            match self
                .client
                .get(url)
                .header("Accept", "application/json")
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => return Ok(resp),
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    last_error = Some(anyhow::anyhow!("HTTP {} - {}", status, text));
                }
                Err(e) => {
                    last_error = Some(anyhow::anyhow!("Request failed: {}", e));
                }
            }

            if attempt < MAX_RETRIES - 1 {
                let wait = 2u64.pow(attempt);
                warn!(attempt = attempt + 1, wait = wait, "fetch failed, retrying");
                tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown error")))
    }

    async fn insert_trades_batch(&self, trades: &[Trade]) -> anyhow::Result<()> {
        let batch: Vec<(String, String, String, i64, String)> = trades
            .iter()
            .map(|trade| {
                let timestamp = trade
                    .created_time
                    .as_ref()
                    .or(trade.ts.as_ref())
                    .cloned()
                    .unwrap_or_default();
                let ticker = trade
                    .ticker
                    .as_ref()
                    .or(trade.market_ticker.as_ref())
                    .cloned()
                    .unwrap_or_default();
                let price_cents = trade
                    .yes_price
                    .as_ref()
                    .or(trade.price.as_ref())
                    .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
                    .unwrap_or(50);
                let price = format!("{:.2}", price_cents as f64 / 100.0);
                let volume = trade.count.or(trade.volume).unwrap_or(1) as i64;
                let taker_side = trade.taker_side.clone().unwrap_or_else(|| {
                    if trade.is_taker_side_yes.unwrap_or(true) {
                        "yes".to_string()
                    } else {
                        "no".to_string()
                    }
                });

                (timestamp, ticker, price, volume, taker_side)
            })
            .collect();

        self.store.insert_historical_trades_batch(&batch).await
    }

    fn market_to_upsert_tuple(
        market: &Market,
    ) -> Option<(String, String, String, String, String, Option<String>)> {
        let ticker = market.ticker.as_deref().map(str::trim).unwrap_or_default();
        if ticker.is_empty() {
            return None;
        }

        let title = market.title.as_deref().unwrap_or(ticker);
        let category = market.category.as_deref().unwrap_or("other");
        let open_time = market
            .open_time
            .as_deref()
            .filter(|s| !s.is_empty())
            .or(market.close_time.as_deref())
            .or(market.expiration_time.as_deref())
            .unwrap_or_default();
        let close_time = market
            .close_time
            .as_deref()
            .or(market.expiration_time.as_deref())
            .unwrap_or_default();

        if open_time.is_empty() || close_time.is_empty() {
            return None;
        }

        let result = normalize_market_result(market.result.as_deref(), market.status.as_deref());
        Some((
            ticker.to_string(),
            title.to_string(),
            category.to_string(),
            open_time.to_string(),
            close_time.to_string(),
            result,
        ))
    }

    async fn load_state(&self, path: &PathBuf) -> anyhow::Result<FetchStateFile> {
        if !path.exists() {
            return Ok(FetchStateFile::default());
        }

        let contents = tokio::fs::read_to_string(path).await?;
        let state: FetchStateFile = serde_json::from_str(&contents)?;
        Ok(state)
    }

    async fn save_state(&self, path: &PathBuf, state: &FetchStateFile) -> anyhow::Result<()> {
        let contents = serde_json::to_string_pretty(state)?;
        tokio::fs::write(path, contents).await?;
        Ok(())
    }

    fn generate_date_range(start: NaiveDate, end: NaiveDate) -> Vec<String> {
        let mut dates = Vec::new();
        let mut current = start;
        while current <= end {
            dates.push(current.format("%Y-%m-%d").to_string());
            current = current.succ_opt().unwrap_or(current);
        }
        dates
    }

    fn date_to_timestamps(date_str: &str) -> anyhow::Result<(i64, i64)> {
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?;
        let start_dt = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| anyhow::anyhow!("Failed to create start datetime"))?;
        let end_dt = date
            .and_hms_opt(23, 59, 59)
            .ok_or_else(|| anyhow::anyhow!("Failed to create end datetime"))?;

        Ok((start_dt.and_utc().timestamp(), end_dt.and_utc().timestamp()))
    }

    pub async fn get_available_data(&self) -> anyhow::Result<DataAvailability> {
        let total_trades = self.store.count_historical_trades().await? as usize;
        let total_markets = self.store.count_historical_markets().await? as usize;
        let has_trades = total_trades > 0;
        let has_markets = total_markets > 0;

        if !has_trades && !has_markets {
            return Ok(DataAvailability {
                has_data: false,
                start_date: None,
                end_date: None,
                total_trades: 0,
                total_markets: 0,
                days_count: 0,
                has_markets: false,
                has_trades: false,
                is_complete: false,
            });
        }

        let summary = if has_trades {
            self.store.get_historical_trades_summary().await?
        } else {
            None
        };
        let (start_date, end_date, days_count) = match summary {
            Some((min_ts, max_ts, days)) => {
                let start = min_ts.split('T').next().map(|s| s.to_string());
                let end = max_ts.split('T').next().map(|s| s.to_string());
                (start, end, days as usize)
            }
            None => (None, None, 0),
        };

        Ok(DataAvailability {
            has_data: has_trades || has_markets,
            start_date,
            end_date,
            total_trades,
            total_markets,
            days_count,
            has_markets,
            has_trades,
            is_complete: has_markets && has_trades,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DataAvailability {
    pub has_data: bool,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub total_trades: usize,
    pub total_markets: usize,
    pub days_count: usize,
    pub has_markets: bool,
    pub has_trades: bool,
    pub is_complete: bool,
}

fn normalize_market_result(result: Option<&str>, status: Option<&str>) -> Option<String> {
    let normalized = result.map(|s| s.trim().to_lowercase());
    match normalized.as_deref() {
        Some("yes") => Some("yes".to_string()),
        Some("no") => Some("no".to_string()),
        Some("cancelled") | Some("canceled") => Some("cancelled".to_string()),
        Some(other)
            if status
                .map(|s| s.eq_ignore_ascii_case("finalized"))
                .unwrap_or(false) =>
        {
            Some(other.to_string())
        }
        _ => None,
    }
}
