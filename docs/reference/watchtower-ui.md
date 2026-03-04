# Watchtower UI Reference

Watchtower is a React-based terminal UI (TUI) built with OpenTUI. It connects to the paper trading engine via WebSocket and displays live garden state.

**Start**: `bun run watchtower` (or `just watchtower`)

**WebSocket URL**: `ws://localhost:3030/ws` (configure via `PM_SERVER_URL` environment variable)

---

## Keyboard navigation

| Key | Action |
|---|---|
| `1` | Garden Overview tab |
| `2` | Current Harvest tab (open positions) |
| `3` | Harvest History tab (closed trades) |
| `4` | Greenhouse Controls tab (enable/disable specimens) |
| `r` | Reconnect WebSocket |
| `q` | Quit |

---

## Tabs

### Garden Overview (`1`)

Displays the specimen tree organized by bed. Each specimen (scorer) shows:

- **Name**: Scorer identifier
- **Status**: Blooming (active) or Dormant (disabled)
- **Recent activity**: Score distribution for the last tick

The overview also shows the pipeline funnel: candidates fetched → filtered → selected → fills in the most recent tick.

This is the primary view for understanding which scorers are contributing to decisions and whether the pipeline is producing activity.

### Current Harvest (`2`)

Open positions. Columns:

| Column | Description |
|---|---|
| Ticker | Market identifier |
| Side | Yes or No |
| Qty | Number of contracts |
| Entry | Entry price per contract |
| Current | Current market price |
| Unreal P&L | Unrealized profit/loss |
| Hold | Time since entry |

Positions are sorted by unrealized P&L descending by default. A position showing red has lost value since entry; green has gained.

### Harvest History (`3`)

Closed trades. Columns:

| Column | Description |
|---|---|
| Ticker | Market identifier |
| Side | Yes or No |
| Qty | Contracts |
| Entry | Entry price |
| Exit | Exit price |
| P&L | Realized profit/loss |
| Exit Reason | Why the position was closed |
| Hold | Duration |

**Exit reasons**:
- `Resolution(Yes/No)` — market resolved
- `TakeProfit` — hit the take profit threshold
- `StopLoss` — hit the stop loss threshold
- `TimeStop` — held beyond `max_hold_hours`
- `ScoreReversal` — score dropped below `score_reversal_threshold`

### Greenhouse Controls (`4`)

Lists all specimens with toggle controls. Enables or disables individual scorers in the running engine. Toggling a specimen to dormant doesn't kill existing positions — it prevents the scorer from contributing to new entries until re-enabled.

Use this to A/B test removing scorers from a live session, or to quickly disable a scorer that appears to be generating bad signals.

### Decision Feed

A scrolling log of real-time pipeline decisions. Each entry shows:

- **Timestamp**
- **Ticker**
- **Action**: `Enter`, `Exit`, or `Skip`
- **Score**: `final_score` at decision time
- **Side**: Yes/No (for Enter decisions)
- **Reason**: Scorer breakdown or exit reason

`Skip` decisions are the majority in a normal session — most markets the pipeline evaluates, it decides not to trade. This is expected. The interesting ones are `Enter` (new position opened) and `Exit` (position closed).

---

## Status bar

The bottom status bar shows:

- **Connection status**: Connected / Disconnected / Reconnecting
- **Portfolio equity**: Current total value (cash + positions)
- **Cash**: Available cash
- **Drawdown**: Current drawdown from equity peak
- **Last tick**: Timestamp of most recent WebSocket update

---

## Connection and fallback behavior

When Watchtower can't connect to `ws://localhost:3030/ws`, it displays demo data (static mock portfolio and specimen tree) so the UI remains usable for development without a running engine. Press `r` to trigger a reconnect attempt.

The demo mode is recognizable by a "DEMO" indicator in the status bar and static, non-updating data.

---

## Themes

Press `t` to cycle through available themes. Theme preferences are persisted between sessions in `watchtower/src/config/persist.ts`.

Themes affect color scheme only — layout and information density are unchanged.

---

## Running without the full engine

For development on Watchtower itself without a running paper engine:

```bash
bun run watchtower
```

Watchtower starts in demo mode automatically. Edit `src/app.tsx` to modify the demo data, or point `PM_SERVER_URL` at a mock WebSocket server.

---

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `PM_SERVER_URL` | `ws://localhost:3030/ws` | WebSocket URL for the engine |

Set in the shell before starting watchtower:

```bash
PM_SERVER_URL=ws://192.168.1.100:3030/ws bun run watchtower
```
