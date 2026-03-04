# Backtesting Methodology

This document explains how the backtester works, what assumptions it makes, and what the output metrics mean.

## How the backtest loop works

The backtester runs a time-stepped simulation. Given a `start_time`, `end_time`, and `interval` (default 1 hour), it steps through time in discrete increments, running the full pipeline at each step.

At each step (`crates/pm-kalshi/src/backtest.rs`):

1. **Resolve positions**: Any market that closed before `eval_time` is resolved. The position is settled at $1.00 (win) or $0.00 (loss) depending on the market result and the side held. Cancelled markets are settled at entry price (no loss, no gain).

2. **Run the pipeline**: `Source → Filter → Scorer → Selector` produces a set of selected candidates with scores.

3. **Generate exit signals**: For currently open positions, check whether any exit rule fires (take profit, stop loss, time stop, score reversal). Exit rules are evaluated against the historical price at `eval_time`.

4. **Execute exits**: Positions with exit signals are closed at the historical price.

5. **Generate entry signals**: From the selected candidates, generate signals for any market we don't already hold.

6. **Execute entries**: Each signal is filled at the historical price plus slippage (default 10 bps). If we can't afford the full quantity at the fill price, we buy what cash allows. If we can't afford any, the signal is dropped.

7. **Record equity snapshot**: Cash + mark-to-market value of all open positions at current prices.

## The historical data source

The `HistoricalMarketSource` queries `HistoricalData` for markets that were active at `eval_time`. "Active" means: the market existed and hadn't resolved yet. The source provides a lookback window (default 24 hours) of price history for each market, which scorers like `MomentumScorer` use to compute trends.

Historical data can come from two sources:
- CSV files in the `data/` directory (loaded directly into memory)
- SQLite database (faster for large datasets, populated via `kalshi ingest`)

## Fill simulation

The backtest fill model (`BacktestExecutor.execute_signal`) is deliberately simple:

1. Look up the Yes price at `eval_time`
2. Compute effective price based on side (No price = 1 - Yes price)
3. Apply slippage: `fill_price = effective_price × (1 + slippage_bps / 10000)`
4. Check against signal limit price — allow up to 5% tolerance over the limit (this accommodates minor bar-close mismatches in historical data)
5. If we have enough cash, fill at the computed quantity. If not, fill what we can afford.

There's no order book simulation. Fills are assumed to go through at the observed price plus slippage. This is a reasonable approximation for Kalshi markets with adequate liquidity, but will overstate performance for illiquid markets or large position sizes.

## Backtest-specific filter settings

The default backtest pipeline uses more permissive filters than paper trading:

- `LiquidityFilter(min_volume=10)` instead of 100 — historical datasets can be sparse
- `TimeToCloseFilter(min_hours=0)` instead of 2 — allows entering markets close to expiry

This is intentional: backtests on historical data would filter out most candidates if paper-trading thresholds were applied, because historical trade data is sampled and doesn't reflect full market activity.

## What the metrics mean

`BacktestResult` contains:

**`total_return_pct`**: `(final_equity - initial_capital) / initial_capital × 100`. Final equity includes cash plus mark-to-market value of any positions still open at `end_time`.

**`sharpe_ratio`**: Annualized Sharpe using per-step returns. Computed as `(mean_return / std_return) × sqrt(252)`. The 252 factor assumes daily steps — if you run with `interval_hours=1`, the Sharpe will be inflated by the square root of 24, because hourly volatility is lower than daily. Use this metric comparatively (same interval) rather than as an absolute number.

**`max_drawdown_pct`**: Peak-to-trough drawdown on the equity curve, as a percentage. Measured at each step, not just on trade closes.

**`win_rate`**: Percentage of *closed* trades that were profitable. Doesn't include still-open positions.

**`avg_hold_time_hours`**: Average time between entry and close, for closed trades. Low values (< 4h) suggest the time stop or exit rules are firing frequently. High values suggest positions are being held to resolution.

**`trades_per_day`**: Total trades (open + close + resolution) divided by duration in days.

**`return_by_category`**: Sum of realized P&L broken down by market category (e.g., "Politics", "Economics"). Useful for identifying which categories your scorers have edge in vs. which are drag.

**`equity_curve`**: List of `{timestamp, equity, cash, positions_value}` at each step. Useful for plotting drawdown and identifying when large losses occurred.

**`trade_log`**: Per-trade records with entry/exit times, prices, and realized P&L. Includes still-open positions (with `exit_time=None`).

## The random baseline

`RandomBaseline` runs the same loop but replaces the pipeline with a deterministic random trader (LCG seeded at 42). It picks a random market from the active set each step and buys a random side at the observed price.

The baseline uses the same position sizing (fixed `max_position_size` contracts), the same resolution logic, and the same loop interval as the strategy being compared.

To compare against the baseline:
```
cargo run --release -p pm-kalshi -- run --start 2024-01-01 --end 2024-06-01 --compare-random
```

A strategy with a Sharpe ratio meaningfully above the baseline's and a win rate above ~55% on binary prediction markets is a signal worth investigating further. Be skeptical of strategy returns that only outperform the baseline by 2-3% — that's well within noise for a 6-month backtest.

## Known limitations

**Look-ahead bias**: The backtest looks up prices at `eval_time` exactly. In practice, you would only know prices slightly before `eval_time` due to API latency. The 10 bps slippage partially compensates for this, but it's not a precise model.

**Sparse historical data**: Kalshi historical data (CSV) contains trade snapshots, not tick data. Price lookback windows for scorers may be based on sparse observations, which can make trend signals noisier than they would be in a live environment.

**No market impact**: A fill for 1,000 contracts on a market with 200 daily volume doesn't model the impact of moving the market. Use the `LiquidityFilter` and `max_position_size` conservatively to stay within realistic fill assumptions.

**No fee simulation in backtest by default**: The `BacktestExecutor` fills do not deduct fees from the portfolio. Paper trading does apply fee drag via `FeeConfig`. This means backtest P&L figures are pre-fee and will overstate realized returns.
