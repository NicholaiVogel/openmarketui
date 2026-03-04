# How to Ingest Historical Data

Backtesting requires historical market data. You can load it two ways: directly from CSV files, or from a SQLite database (faster for large datasets). This guide covers both paths and how to go from raw data to a working backtest.

---

## Data formats

The engine supports three historical data formats:

1. **CSV files** — the raw format from Kalshi data exports or the fetch scripts
2. **SQLite database** — indexed for fast time-range queries, populated via `ingest`
3. **Parquet files** — supported via the web dashboard's data view; not used directly by backtests

---

## Option A: Use CSV files directly

If your data is in CSV files under `data/`:

```bash
cargo run --release -p pm-kalshi -- run \
  --data-dir data \
  --start 2024-01-01 \
  --end 2024-06-01 \
  --capital 10000
```

The `HistoricalData::load(&data_dir)` function reads all CSV files in the directory. This is fine for small datasets (a few hundred MB) but becomes slow for larger ones because it loads everything into memory upfront.

---

## Option B: Ingest to SQLite (recommended for large datasets)

**Step 1: Ingest the CSVs**

```bash
cargo run --release -p pm-kalshi -- ingest \
  --data-dir data \
  --db data/historical.db
```

Or with just:

```bash
just kalshi-ingest
```

You'll see progress and a final count:

```
INFO  ingesting CSV data data_dir=data db=data/historical.db
INFO  ingest complete markets=12841 trades=4819203 db=data/historical.db
```

**Step 2: Run backtests using the database**

```bash
cargo run --release -p pm-kalshi -- run \
  --db data/historical.db \
  --start 2024-01-01 \
  --end 2024-06-01 \
  --capital 10000
```

SQLite queries are indexed on ticker and timestamp, so loading a 6-month slice from a multi-year database is fast.

Re-running ingest on the same directory is safe — it doesn't duplicate entries.

---

## Fetching fresh data from Kalshi

The `tools/` directory contains Python scripts for fetching market data:

```bash
# fetch current market data
python tools/fetch_kalshi_data.py

# or the v2 script with more options
python tools/fetch_kalshi_data_v2.py
```

These scripts write CSV files to `data/`. After fetching, run `ingest` to update the SQLite database if you're using that path.

You'll need Python with the requests library. The scripts call the Kalshi public API — no auth required for market data.

---

## Checking what data you have

After ingest, verify the database contents:

```bash
sqlite3 data/historical.db "
  SELECT
    MIN(timestamp) as earliest,
    MAX(timestamp) as latest,
    COUNT(DISTINCT ticker) as markets,
    COUNT(*) as trades
  FROM historical_trades;
"
```

You can also check the markets table:

```bash
sqlite3 data/historical.db "
  SELECT category, COUNT(*) as count
  FROM historical_markets
  GROUP BY category
  ORDER BY count DESC;
"
```

---

## Troubleshooting

**"loading historical data" errors**: Check that the CSV files are in the expected format. The loader expects trade records with at least `ticker`, `timestamp`, and `yes_price` columns. Category and title columns are optional but improve scorer behavior.

**Low candidate counts in backtest**: If the backtest reports very few candidates each tick, check:
- Your date range falls within the data's actual time span (see the sqlite query above)
- The `LiquidityFilter` threshold isn't too high for your dataset (`min_volume=10` is the backtest default for a reason — historical data is sparse)
- The `TimeToCloseFilter` range includes markets in your dataset

**"data loaded candidates=0"**: This usually means the time range in `--start`/`--end` falls outside your data. Verify with the sqlite query above.
