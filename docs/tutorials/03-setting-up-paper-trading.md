# Tutorial: Setting Up Paper Trading

In this tutorial you'll configure and run a paper trading session against the live Kalshi API. Paper trading uses real market prices and a simulated order book — fills happen at real prices, but no actual money changes hands. By the end you'll have a running engine broadcasting to Watchtower.

**Prerequisites**: Rust and Bun toolchains installed. A `config.toml` configured for your environment. Some historical data ingested (for the web dashboard's historical view).

---

## Step 1: Create config.toml

Copy the example:

```bash
cp fertilizer/kalshi/config.toml.example config.toml
```

If no example exists yet, create `config.toml` at the repo root with the following content. This is the working configuration shape:

```toml
mode = "paper"

[kalshi]
base_url = "https://api.elections.kalshi.com/trade-api/v2"
poll_interval_secs = 60
rate_limit_per_sec = 2

[trading]
initial_capital = 10000.0
max_positions = 20
kelly_fraction = 0.25
max_position_pct = 0.10
take_profit_pct = 0.50
stop_loss_pct = 0.99
max_hold_hours = 48
min_time_to_close_hours = 2
max_time_to_close_hours = 504
cash_reserve_pct = 0.20
max_entries_per_tick = 5

[persistence]
db_path = "/path/to/your/kalshi-paper.db"

[web]
enabled = true
bind_addr = "127.0.0.1:3030"
parquet_data_dir = "/path/to/parquet/data"  # optional

[circuit_breaker]
max_drawdown_pct = 0.15
max_daily_loss_pct = 0.05
max_positions = 20
max_single_position_pct = 0.10
max_consecutive_errors = 5
max_fills_per_hour = 50
max_fills_per_day = 200

[fees]
taker_rate = 0.07
maker_rate = 0.0175
max_per_contract = 0.02
assume_taker = true
min_edge_after_fees = 0.02
```

Key fields to change for your setup:
- `db_path` — where the SQLite database will be written. The directory must exist.
- `parquet_data_dir` — optional path to Parquet market data files for the web dashboard's data view

See [config.toml Reference](../reference/config-toml.md) for all fields.

---

## Step 2: Start the engine

```bash
cargo run --release -p pm-kalshi -- paper --config config.toml
```

Or with just:

```bash
just kalshi-paper
```

You should see startup logs:

```
INFO  starting paper trading mode=Paper poll_secs=60 capital=10000
INFO  starting web dashboard addr=127.0.0.1:3030
INFO  paper trading session started
```

The engine now polls Kalshi every 60 seconds. On each tick, it:
1. Fetches active markets from the API (rate-limited to 2 req/sec)
2. Runs the pipeline to find candidates
3. Generates entry and exit signals
4. Simulates fills via `PaperExecutor`
5. Broadcasts state over WebSocket to `ws://localhost:3030/ws`

---

## Step 3: Start Watchtower

In a new terminal:

```bash
bun run watchtower
```

Watchtower connects to `ws://localhost:3030/ws` and displays the live garden state. You'll see:

- **Garden Overview** (press `1`): The specimen tree showing which scorers are active
- **Current Harvest** (press `2`): Open positions with entry prices and unrealized P&L
- **Harvest History** (press `3`): Closed trades
- **Greenhouse Controls** (press `4`): Enable/disable individual specimens
- **Decision Feed**: Real-time stream of pipeline decisions

If Watchtower shows "disconnected", make sure the engine is running and the web server bound successfully. Check for "port in use" warnings in the engine logs.

---

## Step 4: Watch the first tick

Wait up to 60 seconds for the first poll cycle. You'll see activity in both the engine logs and Watchtower's decision feed. A typical tick looks like:

```
INFO  data loaded candidates=847
INFO  filtered candidates=42 removed=805
INFO  selected candidates=8
INFO  executed trade ticker="KXINFL-24-T3.00" side=Yes quantity=45 price=0.31
```

In Watchtower, the decision feed will show Enter/Skip/Exit actions with their scores.

---

## Step 5: Adjust position limits for safety

For your first paper session, keep positions small while you observe behavior. Edit `config.toml`:

```toml
[trading]
max_positions = 5          # start very small
max_entries_per_tick = 2   # slow down entry rate
cash_reserve_pct = 0.50    # keep 50% in cash

[circuit_breaker]
max_fills_per_hour = 10    # tight limit to catch runaway behavior
max_drawdown_pct = 0.05    # trip early if something is wrong
```

Restart the engine for changes to take effect (it reads config at startup only).

---

## Step 6: View persisted state

The SQLite database at `db_path` contains all fills, positions, decisions, and equity snapshots. You can query it directly:

```bash
sqlite3 /path/to/kalshi-paper.db "SELECT ticker, side, quantity, price FROM fills ORDER BY timestamp DESC LIMIT 20;"
```

Or use the web dashboard at `http://localhost:3030` (if the REST endpoints are enabled).

---

## Step 7: Stop the engine

Press `Ctrl+C`. The engine shuts down gracefully — existing positions are preserved in the database and will be loaded on next startup.

```
INFO  ctrl+c received, shutting down
INFO  paper trading session ended
```

---

## What you've learned

- `config.toml` controls all engine behavior: trading params, persistence, web, circuit breaker, fees
- The paper engine runs on a real clock, polling the Kalshi API at `poll_interval_secs`
- Watchtower connects via WebSocket and provides a live terminal dashboard
- The SQLite database persists all state between sessions
- Circuit breaker limits protect against runaway behavior in early testing

**Next steps**:
- [config.toml Reference](../reference/config-toml.md) — understand every field
- [Monitor a Session in Watchtower](../how-to/monitor-with-watchtower.md) — navigate the TUI effectively
- [Tune the Circuit Breaker](../how-to/tune-circuit-breaker.md) — configure appropriate safety limits
