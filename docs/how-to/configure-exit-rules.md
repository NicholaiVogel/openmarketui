# How to Configure Exit Rules

Exit rules determine when the engine closes an open position. Four independent rules exist: take profit, stop loss, time stop, and score reversal. Any one firing is sufficient to close the position.

---

## In backtesting (CLI flags)

Pass exit parameters directly to the `run` subcommand:

```bash
cargo run --release -p pm-kalshi -- run \
  --start 2024-01-01 --end 2024-06-01 \
  --take-profit 0.30 \
  --stop-loss 0.20 \
  --max-hold-hours 24
```

- `--take-profit 0.30`: exit when unrealized gain reaches 30%
- `--stop-loss 0.20`: exit when unrealized loss reaches 20%
- `--max-hold-hours 24`: force exit after 24 hours regardless of P&L

The `score_reversal_threshold` is not exposed as a CLI flag — it's hardcoded to `-0.3` in backtesting. To change it, modify `main.rs`:

```rust
let exit_config = ExitConfig {
    take_profit_pct: take_profit,
    stop_loss_pct: stop_loss,
    max_hold_hours,
    score_reversal_threshold: -0.3,  // change this
};
```

---

## In paper trading (config.toml)

Edit the `[trading]` section of `config.toml`:

```toml
[trading]
take_profit_pct = 0.50    # exit at +50%
stop_loss_pct = 0.99      # effectively disabled
max_hold_hours = 48
```

`score_reversal_threshold` isn't in the config file currently — it's set to `-0.3` in `main.rs` when building the `ExitConfig`. To change it for paper trading, edit `crates/pm-kalshi/src/main.rs`:

```rust
let exit_config = ExitConfig {
    take_profit_pct: app_config.trading.take_profit_pct.unwrap_or(0.50),
    stop_loss_pct: app_config.trading.stop_loss_pct.unwrap_or(0.99),
    max_hold_hours: app_config.trading.max_hold_hours.unwrap_or(48),
    score_reversal_threshold: -0.3,  // change here
};
```

---

## In code (ExitConfig directly)

If you're building custom pipelines or tests:

```rust
use pm_core::ExitConfig;

// use a preset
let config = ExitConfig::default();           // 50% TP, no SL, 48h, -0.5 reversal
let config = ExitConfig::conservative();      // 15% TP, 10% SL, 48h, -0.2 reversal
let config = ExitConfig::aggressive();        // 30% TP, 20% SL, 120h, -0.5 reversal
let config = ExitConfig::prediction_market(); // 100% TP, no SL, 48h, -0.5 reversal

// or build custom
let config = ExitConfig {
    take_profit_pct: 0.40,
    stop_loss_pct: 0.25,
    max_hold_hours: 72,
    score_reversal_threshold: -0.4,
};
```

---

## Understanding each rule

**Take profit** (`take_profit_pct`): Checks `(current_price - entry_price) / entry_price`. If this reaches the threshold, exit. Locks in gains before the market has a chance to reverse. Default 0.50 works reasonably well for prediction markets — letting winners run past 50% risks giving gains back to resolution uncertainty.

**Stop loss** (`stop_loss_pct`): Same calculation, but for losses. The default `0.99` is effectively disabled. Stop losses tend not to help on binary prediction markets because: (1) prices can gap through a stop between observation windows, and (2) you can't limit your loss below zero anyway — if the market resolves against you, you lose your full position regardless of when you were "stopped out". Use the time stop and score reversal instead.

**Time stop** (`max_hold_hours`): Evaluated first in `compute_exit_signals` — it doesn't even need a current price. Exits positions held longer than the threshold. This is your primary protection against "zombie" positions in illiquid markets that never move enough to hit other rules.

**Score reversal** (`score_reversal_threshold`): Exits when `final_score` for the ticker drops below the threshold. This is the signal-driven exit — it fires when the pipeline changes its mind about a market. A negative threshold (e.g., `-0.3`) means the score must actively turn bearish (not just go neutral) to trigger. This prevents churn from minor score fluctuations.

---

## Choosing values for your strategy

For prediction markets specifically:

- If you're trading short-dated contracts (2–24h to expiry), set `max_hold_hours` conservatively (6–12h) and rely on resolution for normal exits
- If you're trading longer-dated contracts (days to weeks), a larger `max_hold_hours` (72–120h) makes sense, but score reversal becomes more important for catching trend changes
- Stop loss is rarely worth enabling unless you're sizing positions aggressively — the time stop provides more predictable risk control
- Take profit of 0.30–0.50 tends to outperform "hold to resolution" strategies in backtests because it captures asymmetric moves before reversion
