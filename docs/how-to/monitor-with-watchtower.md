# How to Monitor a Session with Watchtower

Watchtower is the terminal UI for observing a live paper trading session. This guide covers practical monitoring workflows for the things you'll actually want to watch.

---

## Start watchtower alongside the engine

The cleanest way is to use the combined task:

```bash
just kalshi-dev
```

This starts the engine and Watchtower together in separate processes. If you want them in separate terminals (for cleaner logs):

```bash
# terminal 1 — engine
just kalshi-paper

# terminal 2 — watchtower
bun run watchtower
```

---

## Watching the first few ticks

When the engine starts, it takes up to `poll_interval_secs` (default 60) before the first tick. In Watchtower, the status bar shows "Connected" once the WebSocket link is established. The decision feed will be empty until the first pipeline run completes.

After the first tick, you'll see the decision feed populate with entries. Most will be `Skip` — that's normal. `Enter` entries are new positions; `Exit` entries are closures.

**What to look for on the first tick**:
- How many candidates were fetched vs. filtered? Check the pipeline funnel in Garden Overview (`1`). If you see `fetched=500 filtered=3`, the filters are very aggressive — possibly because the Kalshi API returned markets but most don't meet your time-to-close or liquidity thresholds.
- Are any specimens showing as dormant when you expect them to be active? Check Greenhouse Controls (`4`).
- Does the portfolio show zero activity? If `Enter` decisions aren't appearing, check the circuit breaker isn't already tripped from a previous session.

---

## Tracking positions

Switch to Current Harvest (`2`) to see open positions. Key things to monitor:

**Position age (Hold column)**: Positions approaching `max_hold_hours` will be force-exited soon. If you see many positions near the time stop, it might mean the score reversal and take profit rules aren't firing often enough — signals are entering but not finding resolution before the time limit.

**Unrealized P&L**: Red positions aren't necessarily a problem. Prediction market prices are noisy. A position down 20% that resolves Yes still returns 100%. The time stop and score reversal protect you from positions that turn into permanent losers.

**Concentration**: If you see many positions in one category, verify your `max_entries_per_tick` and `max_positions` settings are appropriate for your risk tolerance.

---

## Reading the decision feed

The decision feed (`5` or visible in some layouts) shows the engine's reasoning. A healthy decision feed looks like:

```
12:04:22  KXINFL-24-T3.00  Skip   score=0.02  (near-zero score, not worth trading)
12:04:22  KXELEC-24-PA      Skip   score=-0.08 (slight bearish but below threshold)
12:04:22  KXPOL-24-D        Enter  score=0.41  side=Yes  (strong signal, entering)
12:04:22  KXINFL-24-T2.75  Exit   TakeProfit  pnl=+52%
```

**Red flags in the decision feed**:
- Continuous `Enter` activity with no `Skip` entries — the filter is too permissive, or the scorer thresholds are misconfigured
- All `Skip` with no `Enter` for many ticks — filters may be too aggressive, or the threshold for generating a signal is too high
- Many `Exit` entries with `StopLoss` — your stop loss threshold is too tight, or your scorers are generating poor signals
- Many `Exit` entries with `ScoreReversal` — signals are reversing quickly; this is either good (the engine is adaptive) or the price history window is too short (signals are noisy)

---

## Watching for circuit breaker events

The status bar shows drawdown. If drawdown starts climbing:

1. Switch to Harvest History (`3`) to see which closed trades are losing money
2. Look at the category breakdown — is the drag concentrated in one category?
3. Check the decision feed for `Exit` with `StopLoss` or `ScoreReversal` entries — are specific scorers generating bad signals?

If the circuit breaker trips, the engine stops entering new positions. You'll see "Circuit breaker: tripped" in the status bar. To resume, restart the engine (Ctrl+C, then `just kalshi-paper` again). The portfolio state persists, but the circuit breaker resets.

---

## Toggling specimens mid-session

In Greenhouse Controls (`4`), you can enable or disable individual scorers. This is useful for:

- Turning off a scorer you suspect is generating bad signals without restarting the engine
- Testing whether a scorer is contributing positively by disabling it and watching the next few ticks
- Recovering from a situation where one scorer is very aggressively generating entries

Toggling affects only new entry decisions. Existing positions continue to be managed by exit rules regardless of specimen state.

---

## Reconnecting after a disconnect

If Watchtower loses the WebSocket connection (engine restart, network issue), it shows "Disconnected" in the status bar and displays the last known state. Press `r` to reconnect. It reconnects automatically on a timer as well.

If you restart the engine, Watchtower will reconnect and resume displaying live data. Historical data from before the restart won't appear in the decision feed (the feed shows only the current session's messages).
