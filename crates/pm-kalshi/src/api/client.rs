//! Kalshi HTTP client

use super::types::{ApiMarket, ApiTrade, MarketsResponse, TradesResponse};
use crate::config::KalshiConfig;
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;
use std::sync::Arc;
use tracing::{debug, warn};

pub struct KalshiClient {
    http: reqwest::Client,
    base_url: String,
    limiter: Arc<
        RateLimiter<
            governor::state::NotKeyed,
            governor::state::InMemoryState,
            governor::clock::DefaultClock,
        >,
    >,
}

impl KalshiClient {
    pub fn new(config: &KalshiConfig) -> Self {
        let quota = Quota::per_second(
            NonZeroU32::new(config.rate_limit_per_sec).unwrap_or(NonZeroU32::new(5).unwrap()),
        );
        let limiter = Arc::new(RateLimiter::direct(quota));

        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build http client");

        Self {
            http,
            base_url: config.base_url.clone(),
            limiter,
        }
    }

    async fn rate_limit(&self) {
        self.limiter.until_ready().await;
    }

    async fn request_with_retry(&self, url: &str) -> anyhow::Result<reqwest::Response> {
        for attempt in 0..5u32 {
            if attempt > 0 {
                let delay = std::time::Duration::from_secs(3u64.pow(attempt));
                debug!(
                    attempt = attempt,
                    delay_secs = delay.as_secs(),
                    "retrying after rate limit"
                );
                tokio::time::sleep(delay).await;
            }

            self.rate_limit().await;
            let resp = self.http.get(url).send().await?;

            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                warn!(attempt = attempt, "rate limited (429), backing off");
                continue;
            }

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("kalshi API error {}: {}", status, body);
            }

            return Ok(resp);
        }

        anyhow::bail!("exhausted retries after rate limiting")
    }

    pub async fn get_open_markets(&self) -> anyhow::Result<Vec<ApiMarket>> {
        let mut all_markets = Vec::new();
        let mut cursor: Option<String> = None;
        let max_pages = 5;

        for page_num in 0..max_pages {
            self.rate_limit().await;

            let mut url = format!("{}/markets?status=open&limit=200", self.base_url);
            if let Some(ref c) = cursor {
                url.push_str(&format!("&cursor={}", c));
            }

            debug!(url = %url, page = page_num, "fetching markets");

            let resp = self.request_with_retry(&url).await?;
            let page: MarketsResponse = resp.json().await?;
            let count = page.markets.len();
            all_markets.extend(page.markets);

            if !page.cursor.is_empty() && count > 0 {
                cursor = Some(page.cursor);
            } else {
                break;
            }
        }

        debug!(total = all_markets.len(), "fetched open markets");
        Ok(all_markets)
    }

    pub async fn get_market_trades(
        &self,
        ticker: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<ApiTrade>> {
        self.rate_limit().await;

        let url = format!(
            "{}/markets/trades?ticker={}&limit={}",
            self.base_url, ticker, limit
        );

        debug!(ticker = %ticker, "fetching trades");

        let resp = match self.request_with_retry(&url).await {
            Ok(r) => r,
            Err(e) => {
                warn!(ticker = %ticker, error = %e, "failed to fetch trades");
                return Ok(Vec::new());
            }
        };

        let data: TradesResponse = resp.json().await?;
        Ok(data.trades)
    }
}
