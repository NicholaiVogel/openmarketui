# CLI Reference: pm-kalshi

The `pm-kalshi` binary is the main entry point for backtesting and paper trading. Built with:

```bash
cargo build --release -p pm-kalshi
# binary at target/release/pm-kalshi
```

All subcommands are also available via the `just` task runner (see `justfile`).

---

## Subcommands

### run — Backtest

Run a backtest against historical data.

```
cargo run --release -p pm-kalshi -- run [OPTIONS] --start <DATE> --end <DATE>
```

**Required**:

| Flag | Type | Description |
|---|---|---|
| `--start` | string | Start date. Accepts `YYYY-MM-DD` or RFC 3339 (`2024-01-01T00:00:00Z`) |
| `--end` | string | End date. Same format as `--start` |

**Optional**:

| Flag | Type | Default | Description |
|---|---|---|---|
| `--data-dir` / `-d` | path | `data` | Directory containing CSV market data files |
| `--db` | path | — | SQLite database from `ingest`. If provided, skips CSV loading (faster) |
| `--capital` | f64 | `10000` | Initial capital in dollars |
| `--max-position` | u64 | `100` | Maximum contracts per position |
| `--max-positions` | usize | `100` | Maximum concurrent positions |
| `--interval-hours` | i64 | `1` | Simulation step size in hours |
| `--output-dir` | path | `results` | Directory for output files |
| `--compare-random` | flag | off | Run and compare against a random baseline |
| `--kelly-fraction` | f64 | `0.40` | Fractional Kelly multiplier (0.0–1.0) |
| `--max-position-pct` | f64 | `0.30` | Max fraction of cash per position |
| `--take-profit` | f64 | `0.50` | Take profit threshold (e.g., 0.50 = exit at +50%) |
| `--stop-loss` | f64 | `0.99` | Stop loss threshold (0.99 = effectively disabled) |
| `--max-hold-hours` | i64 | `48` | Maximum hours to hold a position before forced exit |

**Output**: Prints a summary to stdout. Saves `backtest_result.json` (and `baseline_result.json` if `--compare-random`) to `--output-dir`.

**Examples**:

```bash
# basic 6-month backtest
cargo run --release -p pm-kalshi -- run \
  --start 2024-01-01 --end 2024-06-01 --capital 10000

# from SQLite (faster for large datasets)
cargo run --release -p pm-kalshi -- run \
  --db data/historical.db \
  --start 2024-01-01 --end 2024-12-31 \
  --capital 50000 --compare-random

# conservative sizing
cargo run --release -p pm-kalshi -- run \
  --start 2024-01-01 --end 2024-06-01 \
  --kelly-fraction 0.10 --max-position-pct 0.05

# just task shortcut (uses defaults from justfile)
just kalshi-backtest
```

---

### ingest — Load CSV to SQLite

Ingest CSV market data files into a SQLite database for faster backtesting.

```
cargo run --release -p pm-kalshi -- ingest [OPTIONS]
```

| Flag | Type | Default | Description |
|---|---|---|---|
| `--data-dir` / `-d` | path | `data` | Source directory with CSV files |
| `--db` | path | `data/historical.db` | Output SQLite database path |

The directory is created if it doesn't exist. Existing database entries are not duplicated — running ingest again on the same data is safe.

**After ingesting**, use `--db data/historical.db` with the `run` subcommand instead of `--data-dir`. This is significantly faster for large datasets because SQLite queries are indexed.

```bash
# ingest CSVs
cargo run --release -p pm-kalshi -- ingest --data-dir data --db data/historical.db

# then backtest using the database
cargo run --release -p pm-kalshi -- run --db data/historical.db \
  --start 2024-01-01 --end 2024-06-01

# just shortcut
just kalshi-ingest
```

---

### paper — Paper trading

Start the paper trading engine using a config file.

```
cargo run --release -p pm-kalshi -- paper [OPTIONS]
```

| Flag | Type | Default | Description |
|---|---|---|---|
| `--config` / `-c` | path | `config.toml` | Path to TOML configuration file |

All paper trading parameters are read from the config file. See [config.toml Reference](config-toml.md).

```bash
# use default config.toml
cargo run --release -p pm-kalshi -- paper

# use a specific config
cargo run --release -p pm-kalshi -- paper --config configs/conservative.toml

# just shortcuts
just kalshi-paper       # engine only
just kalshi-dev         # engine + watchtower together
```

The engine runs until `Ctrl+C`. Portfolio state persists in the SQLite database and is restored on next startup.

---

### summary — Print results

Re-display the summary from a saved backtest results file.

```
cargo run --release -p pm-kalshi -- summary --results-file <PATH>
```

| Flag | Type | Description |
|---|---|---|
| `--results-file` / `-r` | path | Path to `backtest_result.json` |

```bash
cargo run --release -p pm-kalshi -- summary --results-file results/backtest_result.json
```

---

## Logging

Log level is controlled via the `RUST_LOG` environment variable:

```bash
RUST_LOG=info cargo run --release -p pm-kalshi -- run ...
RUST_LOG=debug cargo run --release -p pm-kalshi -- paper  # verbose, includes per-candidate trace
RUST_LOG=kalshi=trace cargo run --release -p pm-kalshi -- paper  # trace level for pm-kalshi only
```

Default is `kalshi=info`.

---

## just tasks

The `justfile` at the repo root defines convenience tasks:

| Task | Command |
|---|---|
| `just kalshi-backtest` | Backtest Jan–Jun 2024, $10k, from `data/` |
| `just kalshi-paper` | Start paper trading with `config.toml` |
| `just kalshi-ingest` | Ingest `data/` CSVs to `data/historical.db` |
| `just kalshi-dev` | Start paper engine + watchtower together |
| `just greenhouse` | Start `pm-server` standalone |
| `just watchtower` | Start watchtower TUI only |
| `just web` | Start the Astro landing page dev server |

Run `just` with no arguments to list all tasks.
