---
id: OpenMarketUI-Phase-One-Daemon-CLI-Parity
aliases: []
tags:
  - domain/project
  - openmarketui
  - phase-one
  - cli
  - daemon
  - parity
created: 2026-04-15
modified: 2026-04-17
title: Phase One Daemon and CLI Parity Audit
---

# Phase One Daemon and CLI Parity Audit

Status: Draft  
Scope: Phase One, Kalshi only  
Primary binary: `omu`  
Human surface: Watchtower / OpenTUI  
Agent surface: Rust CLI

## Decision

The CLI should be written in Rust.

The CLI should not become a second orchestration engine. The daemon is the
runtime authority. Watchtower and `omu` are attached clients that see and
control the same state through the same API.

That gives the system one source of truth:

- The daemon owns live runtime state: session mode, engine state, scorer status,
  current positions, current prices, circuit breaker state, active backtests,
  data fetch progress, and pipeline ticks.
- SQLite owns durable history: fills, equity snapshots, decisions, historical
  market data, audit events, and later calibration data.
- The control API exposes both runtime state and durable history to clients.
- Watchtower and `omu` both attach to the daemon. Neither should reconstruct
  trading state independently.

The practical rule is simple: if Watchtower can see it, `omu` can query it. If
Watchtower can change it, `omu` can change it through the same daemon contract.

## Phase One goal

Phase One is not a strategy expansion phase. It is the trust-building phase for
Kalshi.

The goal is to run the existing Kalshi pipeline at small stakes with enough
observability that a human in Watchtower and an agent in the CLI can diagnose
what the system is doing without guessing.

Phase One succeeds when:

1. The Kalshi daemon can run paper, backtest, and later small-stakes live modes.
2. Watchtower and `omu` share the same daemon state and control API.
3. Historical data fetching is resumable, observable, and diagnosable.
4. Pipeline decisions are persisted and explainable.
5. Backtests, sessions, positions, trades, scorers, and data fetches are
   queryable as JSON.
6. OpenTelemetry traces cover the pipeline path from fetch through decision and
   execution.

## Current state

The workspace already has most of the Kalshi engine pieces:

```txt
crates/pm-core       traits and core trading types
crates/pm-store      SQLite persistence
crates/pm-garden     scorers and filters
crates/pm-engine     execution, sizing, risk, backtesting support
crates/pm-server     older REST/WebSocket server surface
crates/pm-kalshi     Kalshi engine, paper trading, backtesting, web API
watchtower/          OpenTUI human operator surface
```

The current Kalshi binary is `kalshi`, defined in `crates/pm-kalshi`. Its CLI is
narrow:

```txt
kalshi run
kalshi ingest
kalshi paper
kalshi summary
```

The richer operator surface is already inside the daemon/web layer, especially
`crates/pm-kalshi/src/web`:

```txt
GET  /api/status
GET  /api/portfolio
GET  /api/positions
POST /api/positions/{ticker}/close
POST /api/positions/redeem
POST /api/positions/{ticker}/redeem
GET  /api/trades
GET  /api/equity
GET  /api/circuit-breaker
GET  /api/markets
POST /api/control/pause
POST /api/control/resume
POST /api/backtest/run
GET  /api/backtest/status
GET  /api/backtest/result
POST /api/backtest/stop
POST /api/session/start
POST /api/session/stop
POST /api/session/config
GET  /api/session/status
POST /api/data/fetch
GET  /api/data/status
GET  /api/data/available
POST /api/data/cancel
GET  /api/garden/status
GET  /api/beds
GET  /api/beds/{bed}/specimens
POST /api/specimens/{name}/status
POST /api/control/scorers/{name}
PUT  /api/control/weights
GET  /ws
```

Watchtower already consumes this shape. The missing piece is the Rust CLI client
that exposes the same authority to agents.

## Implementation progress

As of 2026-04-17, the first Rust `omu` slice is in place:

- Added `crates/omu` as a workspace binary with JSON-first output envelopes,
  stable error codes, global `--profile`, `--config-dir`, `--daemon-url`,
  `--yes`, and `--dry-run` flags.
- Added daemon-attached commands for overview, daemon lifecycle, portfolio,
  positions, trades, markets, scorer status/control, decisions, ingest,
  backtests, sessions, audit, config, and profiles.
- Added `GET /api/snapshot`, decision routes, audit routes, and graceful daemon
  shutdown to the active `pm-kalshi` web surface.
- Added durable SQLite audit events in `pm-store`.
- Added local profile config at `omu.toml`, with per-profile daemon URLs,
  optional Kalshi config paths, dry-run defaults, live-trading gates, bankroll
  caps, and confirmation policy.
- Made local daemon lifecycle profile-aware through profile-specific state and
  log files.
- Verified dry-run mutation commands are no-ops and do not contact the daemon.
- Added read-only filter registry parity through `GET /api/filters`,
  `GET /api/filters/{name}`, and `omu pipeline filters list|show`.
- Added session creation parity for `omu sessions list|show|create|stop`.
  Paper and backtest session creation use the daemon `/api/session/start`
  contract. Live sessions return `POLICY_BLOCKED` until auth, circuit breaker,
  bankroll, audit, and trace gates exist.
- Added paper-position close parity through `POST /api/positions/{ticker}/close`
  and `omu positions close <ticker>`. The CLI path is dry-run-first,
  confirmation-gated, and audit-recorded.
- Added durable backtest run history through the SQLite `backtest_runs` table,
  `GET /api/backtest/runs`, `GET /api/backtest/runs/{id}`, and
  `omu backtest list|show|compare`. Attached runs include the daemon `run_id`
  so the latest result can be tied back to history.
- Added durable session history through the SQLite `session_runs` table,
  `GET /api/sessions`, `GET /api/sessions/{id}`, and
  `omu sessions list|show`. Paper and backtest session lifecycle is now recorded
  as running, stopped, complete, or failed through the daemon.
- Added redacted auth command parity through `omu auth add|status|rotate` and
  `GET /api/auth/status`. The CLI stores credential metadata locally, never
  prints raw key IDs, and reports daemon live-auth readiness separately.
- Added position redeem controls through `POST /api/positions/redeem`,
  `POST /api/positions/{ticker}/redeem`, and `omu positions redeem`. Bulk
  redeem only claims positions with a daemon-candidate or historical-store
  result; manual ticker redeem is confirmation-gated and audit-recorded by the
  CLI.
- Added trace ID propagation through global `--trace-id` / `OMU_TRACE_ID`,
  generated CLI trace IDs, `x-omu-trace-id` daemon request headers, response
  header echoing, daemon request spans, and persisted audit-event trace IDs.

The next major parity gaps are full OpenTelemetry exporter wiring and
REST/WebSocket response shape alignment.

## Architecture target

```txt
                 shared daemon API
                        │
                        ▼
┌──────────────────────────────────────────────┐
│ Kalshi daemon / orchestration runtime         │
│                                              │
│ - trading sessions                            │
│ - paper/live engine                           │
│ - backtest jobs                               │
│ - historical data fetch jobs                  │
│ - scorer registry and weights                 │
│ - circuit breakers                            │
│ - event stream                                │
│ - SQLite persistence                          │
└──────────────────────────────────────────────┘
              ▲                         ▲
              │                         │
        WebSocket/REST             REST/JSON
              │                         │
              ▼                         ▼
┌──────────────────────┐       ┌──────────────────────┐
│ Watchtower / OpenTUI │       │ omu Rust CLI          │
│ human operator       │       │ agent operator        │
└──────────────────────┘       └──────────────────────┘
```

The CLI can have local responsibilities, but they should be limited:

- Find config and selected profile.
- Locate the daemon endpoint.
- Start, stop, or inspect the daemon process where appropriate.
- Format responses as JSON or human-readable text.
- Enforce confirmation flags for destructive commands.

The CLI should not:

- Run the trading loop itself.
- Maintain its own scorer state.
- Read live positions from SQLite while the daemon has a different in-memory
  state.
- Start backtests in-process when a daemon is available.
- Fork a second path for business logic that Watchtower does not use.

## Recommended crate shape

Phase One can start with one new crate:

```txt
crates/omu/          # Rust CLI binary, thin daemon client
```

Then add a shared API crate if the DTOs start drifting:

```txt
crates/pm-api/       # shared request/response DTOs and stable error codes
```

The `pm-api` crate is not mandatory on day one, but the moment both the daemon
and CLI duplicate more than a few structs, it becomes worth adding. The long-term
shape should be:

```txt
pm-api DTOs ─┬─ used by daemon handlers
             ├─ used by omu client
             └─ exported to TypeScript schemas for Watchtower when needed
```

For Phase One, `crates/omu` can call the existing daemon endpoints with typed
Rust response structs. The important part is keeping `omu` as a client, not as a
second engine.

## Source of truth boundaries

| State | Owner | Clients read through | Notes |
|---|---|---|---|
| Session mode and active config | Daemon memory plus SQLite via daemon | `/api/session/status`, `/api/sessions` | Runtime authority plus durable lifecycle history. |
| Engine status | Daemon memory | `/api/status`, `/ws` | Includes state, uptime, tick count, last tick. |
| Portfolio snapshot | Daemon memory plus store | `/api/portfolio`, `/ws` | Runtime state should win over direct DB reads. |
| Open positions | Daemon memory | `/api/positions`, `/ws` | Durable positions can be restored at daemon boot. |
| Fills and trade history | SQLite via daemon | `/api/trades` | Add show/filter endpoints. |
| Equity curve | SQLite via daemon | `/api/equity` | Existing endpoint is enough for summaries and charts. |
| Pipeline decisions | SQLite via daemon | new `/api/decisions` endpoints | Existing store support exists. Need daemon routes in `pm-kalshi`. |
| Scorer status and weights | Daemon memory | `/api/beds`, `/api/beds/{bed}/specimens`, scorer control endpoints | Later persist presets separately. |
| Backtest state and history | Daemon job state plus SQLite via daemon | `/api/backtest/status`, `/api/backtest/result`, `/api/backtest/runs` | Implemented for latest status/result and durable run history. |
| Historical data fetch state | Daemon job state plus state file | `/api/data/status`, `/api/data/available` | Existing Rust fetcher is already daemon-integrated. |
| Audit log | SQLite via daemon | new `/api/audit` endpoints | Needed for agent trust and destructive command review. |
| OpenTelemetry traces | Collector/exporter | trace backend or local files | Not a CLI source of truth, but CLI should expose trace IDs. |

## CLI command parity matrix

This maps Phase One CLI commands to the current Watchtower or daemon surface.

| CLI command | Purpose | Current support | Gap |
|---|---|---|---|
| `omu overview` | One-shot operator snapshot | WebSocket snapshot already builds this | Add `GET /api/snapshot` or aggregate existing endpoints. |
| `omu daemon status` | Is the daemon reachable and healthy? | Partial via `/api/status` | Add daemon identity, version, profile, config path. |
| `omu daemon start` | Start foreground/background daemon | Existing `kalshi paper` starts engine and web server | Decide whether `omu daemon start` wraps `pm-kalshi` or a new daemon binary. |
| `omu daemon stop` | Stop daemon | Partial session stop exists | Need process-level stop or graceful shutdown endpoint. |
| `omu daemon logs` | Inspect daemon logs | Not exposed | Usually local process manager concern. Can defer. |
| `omu sessions list` | Show known sessions | `/api/session/status`, `/api/sessions` | Implemented as active session plus durable history. |
| `omu sessions show` | Show active/session details | `/api/session/status`, `/api/sessions/{id}` | Implemented for active and historical sessions. |
| `omu sessions create` | Start paper/backtest/live session | `/api/session/start` | Implemented for paper/backtest with durable lifecycle records. Live returns `POLICY_BLOCKED`. |
| `omu sessions stop` | Stop current session | `/api/session/stop` | Implemented and records stopped lifecycle state. |
| `omu portfolio summary` | Cash, equity, return, drawdown | `/api/portfolio` | Existing response lacks realized/unrealized P&L present in WS snapshot. Align shapes. |
| `omu portfolio history` | Equity history | `/api/equity` | Existing. |
| `omu portfolio equity-curve` | Machine-readable equity curve | `/api/equity` | Existing. |
| `omu positions list` | Show open positions | `/api/positions` | Existing. |
| `omu positions show <ticker>` | Drill into one position | List endpoint only | Add filter/show endpoint or client-side filter. |
| `omu positions close <ticker>` | Close a position | `POST /api/positions/{ticker}/close` | Implemented for paper positions. Destructive, requires `--yes`, records audit. |
| `omu positions redeem` | Redeem resolved positions | `POST /api/positions/redeem`, `POST /api/positions/{ticker}/redeem` | Implemented for daemon-candidate or historical-store resolutions and manual paper settlement by ticker. |
| `omu trades list` | Recent fills | `/api/trades` | Existing. |
| `omu trades show <id>` | Fill detail with entry reasoning | No id in response | Add fill IDs and decision linkage. |
| `omu markets list` | Last candidate markets | `/api/markets` | Existing, limited. |
| `omu markets show <ticker>` | Market detail | List endpoint only | Add show endpoint or client-side filter. |
| `omu markets search <query>` | Search candidate/cache markets | Not present | Use `market_cache` or last candidates. |
| `omu pipeline status` | Tick count, funnel, scorer summary | Partial via `/api/status`, WS tick, garden status | Add consolidated pipeline endpoint. |
| `omu pipeline tick` | Force one tick | WS has `ForceRefresh`, not tick | Need engine hook for one pipeline tick if safe. |
| `omu pipeline run` | Start/resume pipeline | `/api/control/resume`, session start | Define semantics around session mode. |
| `omu pipeline stop` | Pause pipeline | `/api/control/pause`, session stop | Define pause vs stop. |
| `omu pipeline scorers list` | List all scorers | `/api/beds`, `/api/beds/{bed}/specimens` | Existing. |
| `omu pipeline scorers show <name>` | Show scorer detail | Bed specimen lookup only | Add show endpoint or client-side lookup. |
| `omu pipeline scorers enable <name>` | Enable scorer | `/api/control/scorers/{name}` | Existing. |
| `omu pipeline scorers disable <name>` | Disable scorer | `/api/control/scorers/{name}` | Existing. |
| `omu pipeline filters list` | List active filters | `/api/filters` | Implemented as read-only active plus available registry. |
| `omu pipeline filters show <name>` | Filter detail | `/api/filters/{name}` | Implemented. |
| `omu pipeline decisions list` | Recent engine decisions | Store supports decisions | Add routes to `pm-kalshi` daemon. |
| `omu pipeline decisions show <id>` | Explain one decision | Store supports lookup | Add route and richer response. |
| `omu backtest run` | Start daemon-managed backtest | `/api/backtest/run` | Implemented with optional `--attach` polling and audit. |
| `omu backtest summary` | Show latest backtest result | `/api/backtest/result` | Implemented. |
| `omu backtest list` | List past backtests | `/api/backtest/runs` | Implemented using durable SQLite history. |
| `omu backtest show <id>` | Inspect one historical run | `/api/backtest/runs/{id}` | Implemented. |
| `omu backtest compare` | Compare runs | `/api/backtest/runs/{id}` | Implemented as CLI-side comparison over daemon history records. |
| `omu ingest fetch` | Start historical data fetch | `/api/data/fetch` | Existing. |
| `omu ingest status` | Fetch progress and data coverage | `/api/data/status`, `/api/data/available` | Existing. |
| `omu config path` | Show config path | Local CLI concern | Add in `omu`. |
| `omu config show` | Show resolved config/profile | Local CLI plus daemon endpoint | Redact secrets. |
| `omu config doctor` | Validate config and daemon connectivity | Local CLI plus daemon health | Add stable checks. |
| `omu profiles list` | List profiles | Local config | Add in `omu`. |
| `omu profiles show` | Show profile policy | Local config | Add in `omu`. |
| `omu profiles create` | Create profile | Local config | Add in `omu`. |
| `omu profiles set-default` | Set default profile | Local config | Add in `omu`. |
| `omu profiles policy` | Show/update policy | Local config | Add in `omu`. |
| `omu auth add` | Add credentials | Local secret/config concern | Implemented with redacted output and local profile metadata. |
| `omu auth status` | Check auth configured | Local plus `/api/auth/status` | Implemented; reports local credential availability and daemon live-auth readiness. |
| `omu auth rotate` | Rotate credentials | Local secret/config concern | Implemented, confirmation-gated, redacted, and audit-attempted when daemon is reachable. |

## Gaps to close first

### 1. Add a Rust `omu` crate

Add a new workspace member:

```txt
crates/omu/
  Cargo.toml
  src/main.rs
  src/client.rs
  src/config.rs
  src/error.rs
  src/output.rs
  src/commands/
```

The CLI should use:

- `clap` for command parsing.
- `reqwest` for HTTP calls to the daemon.
- `serde` and `serde_json` for typed responses.
- `thiserror` or `anyhow` for internal errors.
- Stable error codes for machine handling.

Global flags:

```txt
--format json|human
--profile <name>
--config-dir <path>
--yes
--dry-run
```

The default should be JSON for agent contexts. Human output can exist, but JSON
must be the stable contract.

### 2. Add a daemon snapshot endpoint

Watchtower already gets a full `Snapshot` message over WebSocket. `omu overview`
should not stitch five unrelated endpoints together if the daemon already knows
how to build the canonical snapshot.

Add:

```txt
GET /api/snapshot
```

It should return the same conceptual data as the WebSocket `Snapshot`:

- session
- engine
- portfolio
- positions
- recent fills
- equity curve
- beds and specimens
- circuit breaker
- latest pipeline metrics if available

### 3. Add decisions routes to `pm-kalshi`

`pm-store` already has decision persistence methods. `pm-server` has older
routes for decisions. The active `pm-kalshi` daemon should expose them too:

```txt
GET /api/decisions?limit=100
GET /api/decisions/{id}
GET /api/markets/{ticker}/decisions?limit=100
```

The decision response should include:

- id
- timestamp
- ticker
- action
- side
- score
- confidence
- scorer breakdown
- reason
- signal id if any
- fill id if any
- latency

This is the core agent debugging path.

### 4. Align REST and WebSocket response shapes

Some fields exist in the WebSocket snapshot but not the REST endpoints. For
example, `PortfolioSnapshot` includes realized, unrealized, and total P&L, while
`PortfolioResponse` does not.

Phase One should make REST and WebSocket shapes match where they represent the
same concept. That reduces drift between Watchtower and `omu`.

### 5. Add filter registry endpoints

The spec requires:

```txt
omu pipeline filters list
omu pipeline filters show
```

There is no daemon endpoint for this yet. Phase One should expose a read-only
registry first:

```txt
GET /api/filters
GET /api/filters/{name}
```

Runtime enable/disable can come later if filters are not dynamically controlled
today.

### 6. Add audit events

Any command that changes daemon state should write an audit event:

- session start
- session stop
- pause/resume
- scorer enable/disable
- scorer weight change
- backtest start/stop
- data fetch start/cancel
- position close
- auth/profile/policy changes where applicable

The audit event should include:

- timestamp
- actor, initially `cli`, `watchtower`, or `daemon`
- command
- profile
- dry-run flag
- request summary
- result
- trace id if available

This matters because agents need a reviewable action trail.

### 7. Add OpenTelemetry boundaries now

Phase One should add spans around:

- CLI command execution
- daemon request handling
- session start/stop
- data fetch day and page fetch
- pipeline tick
- source fetch
- filter stage
- scorer stage
- selector stage
- execution stage
- decision persistence
- fill persistence
- backtest run and step loop

The CLI should print or include trace IDs in JSON output when available.

## Suggested implementation order

### Step 1: Scaffold the Rust CLI

Create `crates/omu` and add it to the workspace. Implement only local plumbing
and daemon health first:

```txt
omu daemon status
omu config path
omu config doctor
```

`omu daemon status` should call `/api/status` and `/api/session/status`.

### Step 2: Add output and error contracts

All JSON responses should follow a stable envelope:

```json
{
  "ok": true,
  "data": {},
  "meta": {
    "profile": "default",
    "daemon_url": "http://127.0.0.1:3030",
    "trace_id": "4f0b4f5e2df74ed895b7c5a5dcaf2a2b"
  }
}
```

Errors should be stable:

```json
{
  "ok": false,
  "error": {
    "code": "DAEMON_UNAVAILABLE",
    "message": "could not connect to daemon at http://127.0.0.1:3030",
    "hint": "start it with `omu daemon start` or set the daemon URL in your profile"
  }
}
```

### Step 3: Implement read-only parity

Implement commands that cannot change trading state:

```txt
omu overview
omu sessions show
omu portfolio summary
omu portfolio equity-curve
omu positions list
omu positions show <ticker>
omu trades list
omu markets list
omu pipeline scorers list
omu pipeline decisions list
omu ingest status
omu backtest summary
```

Where routes are missing, add daemon routes first rather than making the CLI read
SQLite directly.

### Step 4: Implement controlled mutations

Add commands that change daemon state, all through daemon endpoints:

```txt
omu sessions create
omu sessions stop
omu pipeline run
omu pipeline stop
omu pipeline scorers enable <name>
omu pipeline scorers disable <name>
omu ingest fetch
omu backtest run
omu backtest stop
```

Destructive or capital-affecting commands must require `--yes` unless the daemon
is in dry-run mode.

### Step 5: Add daemon lifecycle

Decide whether Phase One uses the existing `pm-kalshi` daemon path or introduces
a dedicated daemon binary.

Two viable paths:

1. Short path: `omu daemon start` wraps the existing `kalshi paper --config ...`
   process.
2. Cleaner path: add a daemon subcommand or crate that owns the runtime directly,
   then make `kalshi` legacy/backcompat.

The cleaner path matches the product spec better, but the short path gets the CLI
attached faster.

## MVP command set

The first useful `omu` should be small but real:

```txt
omu daemon status
omu overview
omu portfolio summary
omu positions list
omu trades list
omu pipeline scorers list
omu pipeline decisions list
omu ingest status
omu ingest fetch --start 2024-01-01 --end 2024-06-01
omu backtest run --start 2024-01-01 --end 2024-06-01 --capital 10000 --attach
omu backtest summary
omu sessions show
omu sessions stop --yes
```

That is enough for an agent to answer:

- Is the daemon alive?
- What mode is the engine in?
- What is the account state?
- What positions are open?
- What trades happened?
- What did the scorers do?
- Why did the engine enter, exit, or skip?
- Is historical data available?
- Is a backtest running, done, or failed?

## Live capital gate

Phase One should not allow live execution just because the command tree exists.

Live trading should require all of the following:

- profile policy explicitly permits live mode
- daemon reports live mode support
- credentials pass auth check
- circuit breaker is healthy
- max position and bankroll caps are set
- command includes `--yes`
- audit event is written
- OTel trace is emitted

Until then, `omu sessions create --mode live` should return a stable
`NOT_IMPLEMENTED` or `POLICY_BLOCKED` error.

## Open questions

1. Should the daemon endpoint be HTTP-only for Phase One, or should `omu` prefer a
   Unix socket when local?
2. Should `omu daemon start` run the daemon in the foreground by default, with
   `--detach` for background operation?
3. Should CLI config live in OpenMarketUI config, or in a separate
   `~/.config/openmarketui/omu.toml` profile file?
4. Should Watchtower consume generated TypeScript types from Rust DTOs, or is
   manual type sync acceptable until the API stabilizes?
5. Should `pm-server` be folded into `pm-kalshi` or replaced by a dedicated
   daemon crate before Phase Two?

## Recommended next move

Keep the short path and close the remaining trust gates:

1. Finish OpenTelemetry exporter wiring beyond the request/audit trace ID slice,
   especially session lifecycle, backtests, execution, decisions, and fills.
2. Continue aligning REST and WebSocket response shapes so Watchtower and `omu`
   stay attached to the same source of truth.

This keeps the system honest: one daemon, two attached operator surfaces, no
split-brain trading logic.
