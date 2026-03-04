# Tutorial: Your First Backtest

In this tutorial you'll run a backtest against historical Kalshi market data, observe the output, and understand what the results mean. By the end you'll have run your first strategy evaluation and seen where to look for what it tells you.

**Prerequisites**: Rust toolchain installed, `cargo` available, historical CSV data in `data/`.

If you don't have data yet, skip to [How to Ingest Historical Data](../how-to/ingest-historical-data.md) first, then come back.

---

## Step 1: Build the binary

From the repo root:

```bash
cargo build --release -p pm-kalshi
```

The binary lands at `target/release/pm-kalshi`. For convenience, the `just` task runner wraps it:

```bash
just kalshi-backtest
```

But let's run it directly so you can see all the flags.

---

## Step 2: Run a basic backtest

```bash
cargo run --release -p pm-kalshi -- run \
  --data-dir data \
  --start 2024-01-01 \
  --end 2024-06-01 \
  --capital 10000
```

This runs the default strategy (momentum + mean reversion + volume scorers via `CategoryWeightedScorer`) over six months of data, starting with $10,000.

You'll see structured log output as the backtest runs:

```
2024-01-01T00:00:00Z INFO  starting backtest total_steps=4416
2024-01-01T01:00:00Z INFO  market resolved ticker="KXINFL-23-T2.75" result=Yes pnl=...
2024-01-01T02:00:00Z INFO  executed trade ticker="KXELEC-..." side=Yes quantity=100 price=0.32
...
```

When it finishes, you'll see the summary:

```
backtest results
================

performance
-----------
total return:     $412.33 (4.12%)
sharpe ratio:     0.847
max drawdown:     8.21%

trades
------
total trades:     284
win rate:         54.3%
avg trade pnl:    $1.45
avg hold time:    18.3 hours
trades per day:   1.56

by category
-----------
  Economics: $218.40
  Politics: $94.12
  Sports: -$0.19
```

Results are also written to `results/backtest_result.json`.

---

## Step 3: Compare against the random baseline

To know if your strategy is actually doing something useful, compare it to a random trader on the same data:

```bash
cargo run --release -p pm-kalshi -- run \
  --data-dir data \
  --start 2024-01-01 \
  --end 2024-06-01 \
  --capital 10000 \
  --compare-random
```

After the strategy results, you'll see:

```
--- random baseline ---

backtest results
================
total return:     $-182.10 (-1.82%)
sharpe ratio:     0.201
...

--- comparison ---

strategy return: 4.12% vs baseline: -1.82%
strategy sharpe: 0.847 vs baseline: 0.201
strategy win rate: 54.3% vs baseline: 49.1%
```

A strategy that outperforms random is a start. Whether that outperformance is real or a backtest artifact requires more investigation — see [Backtesting Methodology](../explanation/backtesting-methodology.md).

---

## Step 4: Adjust the parameters

Try different exit rules to see how they affect results:

```bash
# more aggressive take profit (exits at 30% gain instead of 50%)
cargo run --release -p pm-kalshi -- run \
  --data-dir data \
  --start 2024-01-01 \
  --end 2024-06-01 \
  --capital 10000 \
  --take-profit 0.30 \
  --max-hold-hours 24
```

Try a more conservative Kelly fraction:

```bash
cargo run --release -p pm-kalshi -- run \
  --data-dir data \
  --start 2024-01-01 \
  --end 2024-06-01 \
  --capital 10000 \
  --kelly-fraction 0.10 \
  --max-position-pct 0.05
```

See the [CLI Reference](../reference/cli-reference.md) for all available flags.

---

## Step 5: Read the output file

The JSON output at `results/backtest_result.json` contains the full equity curve, per-trade log, and category breakdown. To re-display the summary later:

```bash
cargo run --release -p pm-kalshi -- summary --results-file results/backtest_result.json
```

---

## What you've learned

- The `run` subcommand is the entry point for backtesting
- The backtest runs the default pipeline (Source → Filter → Scorer → Selector → Executor) stepped hourly
- The summary shows performance (return, Sharpe, drawdown) and trade stats (win rate, hold time)
- `--compare-random` provides a sanity check baseline
- Results are saved to JSON for later analysis

**Next steps**:
- [Building a Custom Scorer](02-building-a-custom-scorer.md) — add your own signal to the pipeline
- [Backtesting Methodology](../explanation/backtesting-methodology.md) — understand what these numbers actually mean
- [CLI Reference](../reference/cli-reference.md) — full parameter documentation
