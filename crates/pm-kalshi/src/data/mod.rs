//! Historical data loading for backtesting

mod fetcher;
mod loader;

pub use fetcher::{DataAvailability, DataFetcher, FetchProgress, FetchState, FetchStatus};
pub use loader::{ingest_csv_to_sqlite, HistoricalData};
