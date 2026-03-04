# How to Tune the Circuit Breaker

The circuit breaker stops the engine from entering new positions when portfolio risk thresholds are exceeded. This guide covers how to configure it appropriately for different risk tolerances.

---

## Default configuration

The defaults in `config.toml`:

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

These defaults are deliberately loose. They're designed to catch severe bugs or runaway behavior, not to actively manage normal trading risk. For active paper trading, you should tighten them.

---

## Tightening for early testing

When you're first running paper trading and want maximum safety:

```toml
[circuit_breaker]
max_drawdown_pct = 0.05       # trip at 5% drawdown from peak
max_daily_loss_pct = 0.02     # trip at 2% daily loss
max_positions = 10             # hard cap at 10 concurrent positions
max_single_position_pct = 0.05 # no single position > 5% of equity
max_consecutive_errors = 3    # trip faster on API errors
max_fills_per_hour = 10       # something is wrong if you're filling 10/hour
max_fills_per_day = 50
```

With these settings, the circuit breaker trips on any unusual behavior. You'll restart the engine more often, but you'll catch problems early.

---

## Tuning for active paper trading

Once you've validated the engine is behaving correctly over several sessions:

```toml
[circuit_breaker]
max_drawdown_pct = 0.10       # trip at 10% drawdown
max_daily_loss_pct = 0.03     # 3% daily loss limit
max_positions = 25             # reasonable for $10k capital
max_single_position_pct = 0.08
max_consecutive_errors = 5
max_fills_per_hour = 30       # realistic max for normal operation
max_fills_per_day = 200
```

---

## What each limit does

**`max_drawdown_pct`**: Measured from the equity peak in the current session. When `(peak_equity - current_equity) / peak_equity >= threshold`, the circuit breaker trips. This catches sequences of losing trades before they compound into a large loss.

Set this to 2× your expected max drawdown from backtesting. If backtests show a worst-case drawdown of 8%, set `max_drawdown_pct = 0.16`. This gives headroom for the live environment being harder than the backtest while still protecting against catastrophic sequences.

**`max_daily_loss_pct`**: Resets daily. Measured as `(starting_equity - current_equity) / starting_equity` since the start of the calendar day. This prevents a single bad day from compounding.

**`max_positions`**: Hard cap on concurrent positions. Separate from `[trading].max_positions` — the effective limit is whichever is lower. Circuit breaker limit should generally be the higher of the two (it's a safety net, not the operational limit).

**`max_single_position_pct`**: Rejects any new entry that would put more than this fraction of equity into a single market. This catches a bug where Kelly sizing returns an abnormally large number.

**`max_consecutive_errors`**: Counts sequential failures from the API or executor. If the Kalshi API is down or returning errors, this trips before the engine starts generating log spam. Check `max_consecutive_errors = 3` for early protection, `5` for tolerance of transient failures.

**`max_fills_per_hour` / `max_fills_per_day`**: Primarily bug detection. With `poll_interval_secs = 60` and `max_entries_per_tick = 5`, you'd expect at most `5 fills/minute = 300 fills/hour` in an extremely active session. In practice, fill rates are much lower. If you see fills approaching `max_fills_per_hour`, something is wrong.

---

## Monitoring circuit breaker state

In Watchtower, the status bar shows current drawdown. The engine logs circuit breaker events at `WARN` level:

```
WARN  circuit breaker triggered rule=max_drawdown_pct value=0.163 threshold=0.15
INFO  circuit breaker: no new entries until reset
```

Circuit breaker events are also persisted to the `circuit_breaker_events` table in SQLite.

---

## What happens when the circuit breaker trips

The engine **stops entering** new positions. It does **not**:
- Close existing positions (those continue to be managed by exit rules)
- Disconnect from the API
- Stop logging or broadcasting to Watchtower

Existing positions will continue to close via take profit, stop loss, time stop, or resolution. After the breaker trips, you can watch the portfolio wind down naturally without needing to intervene.

To resume trading, restart the engine. The circuit breaker resets at startup; portfolio state is restored from the database.

---

## The circuit breaker and paper trading

Paper trading is the right place to tune these values. The consequence of a tripped circuit breaker in paper is just a restart — there's no real money at stake. Use paper sessions to develop an intuition for:

- What your normal max drawdown looks like (set the breaker at 2×)
- What your normal fills-per-day looks like (set the fill limits at 3–5×)
- How often API errors occur in practice (set `max_consecutive_errors` accordingly)

Don't use the defaults blindly if you ever move toward live trading. The defaults are engineering minimums, not risk management policy.
