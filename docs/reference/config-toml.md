# config.toml Reference

`config.toml` is the configuration file for paper trading. It's read at startup by `AppConfig::load()` in `crates/pm-kalshi/src/config/mod.rs`. Changes take effect on the next engine restart.

Backtesting does not use `config.toml` — those parameters are passed as CLI flags. See the [CLI Reference](cli-reference.md).

---

## Top-level

```toml
mode = "paper"
```

**`mode`**: Currently only `"paper"` is supported at this level. Reserved for future live trading mode.

---

## [kalshi]

```toml
[kalshi]
base_url = "https://api.elections.kalshi.com/trade-api/v2"
poll_interval_secs = 60
rate_limit_per_sec = 2
```

**`base_url`**: Kalshi API base URL. Don't change this unless Kalshi updates their API endpoint.

**`poll_interval_secs`**: How often the engine fetches fresh market data and runs the full pipeline. Each poll cycle is one "tick". Lower values increase responsiveness but also API load. Minimum recommended: 30 seconds.

**`rate_limit_per_sec`**: Maximum API requests per second to the markets endpoint. Kalshi's documented limit is 2 req/sec for this endpoint. Do not increase beyond 2 without checking Kalshi's current rate limit documentation.

---

## [trading]

```toml
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
```

**`initial_capital`**: Starting capital in dollars. Loaded into the portfolio on first run. On subsequent runs (existing database), the persisted portfolio state is used instead.

**`max_positions`**: Maximum concurrent open positions. New entries stop generating once this is reached. Also configurable in `[circuit_breaker]` — the effective limit is the minimum of the two.

**`kelly_fraction`**: Fractional Kelly multiplier (0.0–1.0). Applied to the raw Kelly bet size. 0.25 is conservative; 0.40 is the backtest default. See [Position Sizing](../explanation/position-sizing.md).

**`max_position_pct`**: Hard cap on any single position as a fraction of available cash. 0.10 means no position can consume more than 10% of cash. Applied after Kelly sizing.

**`take_profit_pct`**: Exit a position when unrealized gain reaches this percentage. 0.50 = exit at +50%.

**`stop_loss_pct`**: Exit a position when unrealized loss reaches this percentage. 0.99 effectively disables stop losses (a -99% loss on a prediction market contract means the market resolved against you, at which point you've already lost everything — the stop fires at resolution). Set to a real value like 0.30 if you want to cut losses before resolution.

**`max_hold_hours`**: Exit a position after holding this many hours, regardless of P&L. Prevents positions from sitting indefinitely on illiquid or slow-moving markets.

**`min_time_to_close_hours`**: The `TimeToCloseFilter` minimum. Markets closing in less than this many hours are filtered out. Avoids entering markets with no time to develop edge.

**`max_time_to_close_hours`**: The `TimeToCloseFilter` maximum. Markets closing farther out than this are filtered out. 504 hours = 21 days.

**`cash_reserve_pct`**: Fraction of capital kept as a permanent cash reserve. The engine won't deploy the last `cash_reserve_pct × initial_capital` dollars. 0.20 = keep at least 20% in cash always.

**`max_entries_per_tick`**: Maximum new positions opened in a single poll cycle. Prevents a burst of entries when many candidates score well simultaneously.

---

## [persistence]

```toml
[persistence]
db_path = "/path/to/kalshi-paper.db"
```

**`db_path`**: Absolute path to the SQLite database. The file is created if it doesn't exist. The directory must already exist.

The database persists: fills, positions, equity snapshots, decisions, circuit breaker events, and pipeline run metrics. It's safe to keep the same database across restarts.

---

## [web]

```toml
[web]
enabled = true
bind_addr = "127.0.0.1:3030"
parquet_data_dir = "/path/to/parquet/data"
```

**`enabled`**: Whether to start the web/WebSocket server. Set to `false` if you're running headless and don't need Watchtower or the REST API.

**`bind_addr`**: Address and port for the server. Default `127.0.0.1:3030` is local-only. Use `0.0.0.0:3030` to expose on all interfaces (for remote Watchtower access).

**`parquet_data_dir`**: Optional. Path to a directory containing Parquet market data files. Exposed via the web dashboard's data view. If absent or the path doesn't exist, the data view is unavailable.

---

## [circuit_breaker]

```toml
[circuit_breaker]
max_drawdown_pct = 0.15
max_daily_loss_pct = 0.05
max_positions = 100
max_single_position_pct = 0.10
max_consecutive_errors = 5
max_fills_per_hour = 500
max_fills_per_day = 2000
```

**`max_drawdown_pct`**: Trip the circuit breaker if portfolio equity drops more than this percentage from its peak. 0.15 = trip at 15% drawdown. Once tripped, no new entries are made until manually reset (engine restart).

**`max_daily_loss_pct`**: Trip if total P&L for the current calendar day exceeds this percentage of starting equity.

**`max_positions`**: Hard cap on concurrent positions. Separate from `[trading].max_positions` — both apply; the effective limit is the lower of the two.

**`max_single_position_pct`**: Rejects any entry that would put more than this fraction of equity into a single position.

**`max_consecutive_errors`**: Trip if the API or executor returns errors this many times in a row. Prevents the engine from hammering a failing API endpoint.

**`max_fills_per_hour`** and **`max_fills_per_day`**: Trip if fill count exceeds these limits. These exist to catch bugs where the engine generates runaway entries. In normal operation with default settings, fills per hour should be well under 20.

---

## [fees]

```toml
[fees]
taker_rate = 0.07
maker_rate = 0.0175
max_per_contract = 0.02
assume_taker = true
min_edge_after_fees = 0.02
```

**`taker_rate`**: Kalshi's taker fee as a fraction. 0.07 = 7% of profit. Current as of the config in this repo — verify against Kalshi's fee schedule.

**`maker_rate`**: Maker fee. Lower than taker. 0.0175 = 1.75%.

**`max_per_contract`**: Fee cap per contract. Kalshi caps fees at $0.02 per contract for some market types.

**`assume_taker`**: Whether to use taker rate for fee calculations. Set to `true` unless you're placing limit orders well inside the spread (unusual for this engine).

**`min_edge_after_fees`**: Minimum required edge after fee drag to generate a signal. 0.02 = require at least 2 points of net edge. Signals below this are dropped before submission, preventing fee-losing trades with marginal edge.
