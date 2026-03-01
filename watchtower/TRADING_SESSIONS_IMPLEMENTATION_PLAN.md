TRADING_SESSIONS_IMPLEMENTATION_PLAN
===================================

Goal
----

From the Watchtower TUI (`watchtower/`), the user can:

- Switch trading mode: `paper`, `backtest`, `live`
- See the active mode in the header (authoritative, from server session)
- Start/stop the selected mode
- Have the existing UI panels populate from the active session:
  - positions
  - fills
  - decisions
  - timeline
  - rhythm

MVP Target
----------

Use `crates/pm-kalshi` as the web server for session control and data.

- REST base: `http://localhost:3030`
- WebSocket: `ws://localhost:3030/ws`
- Watchtower env: `PM_SERVER_URL=ws://localhost:3030/ws`

Key Findings / Current Gaps
--------------------------

1) Wire format mismatch (most critical)

`pm-kalshi` WebSocket + REST responses are `snake_case`, while Watchtower
types/components largely expect `camelCase`.

Examples:

- Server sends `recent_fills`, UI reads `recentFills`
- Server sends `entry_price`, UI reads `entryPrice`
- Server sends `total_pnl`, UI reads `totalPnl`
- Session status REST uses `trading_active`, UI expects `tradingActive`

This causes the UI to receive data but not render it correctly.

2) Mode switching flow violates server constraint

`POST /api/session/start` returns `409 CONFLICT` if a session is already running.
The UI currently can attempt to start a new mode without fully stopping the
current session first.

3) Decisions are not emitted over WebSocket (paper/backtest/live)

Watchtower expects `type: "Decision"` messages (see `watchtower/src/types/ws.ts`
and reducer in `watchtower/src/hooks/useGardenStore.ts`), but
`crates/pm-kalshi/src/web/ws.rs` does not currently emit any Decision messages.

As a result, DecisionFeed/Timeline do not populate during live/paper sessions.

4) Backtests populate fills/equity via REST polling, but not decisions

Watchtower syncs backtest results into the garden store (`syncFromBacktest`),
converting `trade_log` into fills and `equity_curve` into equity, but it leaves
`decisions` empty. This makes the UI feel "half populated" for backtests.

Implementation Strategy
-----------------------

Boundary-first: fix serialization/normalization at the boundaries so the
existing UI becomes "data-correct" without a deep refactor.

Then: fix the session lifecycle semantics (stop-then-start), then add
Decision telemetry.

Phased Plan
-----------

Phase 0: Define the contract

- Document a canonical "Watchtower UI model" (camelCase) and a canonical
  "pm-kalshi wire model" (snake_case), and explicitly map between them.
- Confirm which fields are authoritative:
  - header mode indicator must come from the server session info envelope
    carried in WS messages (Welcome/Snapshot/TickUpdate)
  - UI selection is a request, not truth

Phase 1: Fix wire -> UI normalization in Watchtower

Objective:

- Positions, portfolio, fills, equity curve display correctly for paper sessions
  via WS Snapshot/TickUpdate.
- Session status display reflects reality.

Changes (Watchtower):

1) Add a normalization module (new file)

- `watchtower/src/wire/normalize.ts` (or similar)
- Export small pure functions:
  - `normalizeSessionInfo(session: any): SessionInfo`
  - `normalizePortfolio(portfolio: any): PortfolioSnapshot`
  - `normalizePosition(p: any): Position`
  - `normalizeFill(f: any): Fill`
  - `normalizeEquityPoint(p: any): EquityPoint`
  - `normalizeServerMessage(msg: any): ServerMessage`

Design rules:

- Keep the normalization shallow and explicit (avoid magical case-conversion).
- Handle missing fields defensively with safe defaults.
- Do not change UI component code; adapt data at ingress.

2) Apply WS normalization at ingress

- Update `watchtower/src/hooks/useWebSocket.ts` to normalize parsed JSON
  before passing to `handleMessage`.
  - Today: `const data = JSON.parse(event.data) as ServerMessage;`
  - After: `const data = normalizeServerMessage(JSON.parse(event.data));`

3) Apply normalization inside `useGardenStore.handleMessage`

- If we normalize inside `useWebSocket`, store reducer remains clean.
- Alternatively normalize inside the reducer per message type.

Pick one: normalize in `useWebSocket` (preferred) so all message handling uses
the same normalized shape.

4) Normalize REST session status

- Update `watchtower/src/hooks/useModeStore.ts` `fetchSessionStatus()` to map
  the REST fields to UI fields (e.g. `trading_active` -> `tradingActive`).

Verification:

- Start pm-kalshi web server + watchtower; confirm:
  - header shows connected mode accurately
  - CurrentHarvest shows positions with titles/categories
  - HarvestHistory shows fills with fee/pnl if present
  - TradingRhythm renders when fills exist

Phase 2: Correct mode lifecycle semantics (stop-then-start)

Objective:

- Switching modes never produces a server-side 409.
- Header updates correctly during transition.

Changes (Watchtower):

1) Centralize the transition logic

- Add a single action in `watchtower/src/hooks/useModeStore.ts`:
  - `transitionToMode(target: TradingMode): Promise<void>`

Behavior:

- If a session is running in a different mode:
  - call `stopSession()`
  - wait briefly (or poll `/api/session/status` until idle)
  - set `viewMode` to target
  - start or open config depending on target

2) Remove duplicated mode switching logic in UI

- Update:
  - `watchtower/src/components/layout/ModeSelector.tsx`
  - `watchtower/src/hooks/useKeyboardNav.ts`
  to call `transitionToMode`.

3) Make header authoritative

- Ensure `viewMode` displayed in `watchtower/src/components/layout/Header.tsx`
  is driven by the server session envelope (WS Welcome/Snapshot/TickUpdate).

Options:

- Option A (recommended): Store an `authoritativeSession` in `useModeStore` and
  update it whenever WS messages arrive.
- Option B: Keep current store but on each WS Snapshot/TickUpdate, set
  `useModeStore.setState({ viewMode: msg.session.mode, sessionStatus: ... })`.

Phase 3: Make backtests populate the full UI (including decisions)

Objective:

- After a backtest completes, DecisionFeed and Timeline have content.

Changes (Watchtower):

1) Extend `watchtower/src/hooks/useGardenStore.ts` `syncFromBacktest()`

- Synthesize a minimal decisions stream from backtest trade log:
  - For each trade, emit an `enter` decision at entry_time
  - If it has an exit, emit an `exit` decision at exit_time

Fields:

- `ticker`, `timestamp`, `action`, `side`, `score`, `confidence`,
  `scorerBreakdown`, `reason`

Notes:

- Backtests currently do not expose per-candidate scorer breakdown; set
  `scorerBreakdown={}` and `confidence=0` (or a fixed heuristic).
- Keep IDs stable (e.g. `${ticker}:${timestamp}:${action}`) for selection.

2) Ensure fills/equity ordering

- Existing fill sorting is ascending; ensure UI components that assume
  descending order either sort or accept the current ordering consistently.

Phase 4: Add real decision telemetry to pm-kalshi (paper + backtest + live)

Objective:

- DecisionFeed/Timeline update in real-time during paper/live sessions.
- Decisions are persisted for later retrieval.

Changes (pm-kalshi):

1) Emit Decision messages over WebSocket

- Update `crates/pm-kalshi/src/web/ws.rs`:
  - Add `ServerMessage::Decision { ... }` variant matching Watchtower's
    `DecisionMessage` shape (type `"Decision"`, camel/underscore consistent
    with chosen normalization approach).

2) Record decisions to SQLite

- Use `pm_store::SqliteStore::record_decision(&pm_core::Decision)`.
- Ensure the engine constructs decisions at the right times.

3) Generate decisions in the engine tick loop

- Update `crates/pm-kalshi/src/engine/trading.rs`:
  - For each exit signal executed: emit a Decision(exit)
  - For each entry signal attempted:
    - emit Decision(enter) if fill occurs
    - optionally emit Decision(skip) when a high-score candidate is rejected
      due to fees/cash reserve/max positions

4) Broadcast decisions

- After persisting (or even if persistence fails), broadcast
  `ServerMessage::Decision` on `updates_tx` so connected clients update.

Design constraints:

- Keep the decision schema small and stable; Watchtower only needs:
  - action, side, score, confidence, scorer_breakdown, reason, latency

Phase 5: Live mode (deferred until after MVP stabilization)

Objective:

- `SessionMode::Live` should start a real executor or explicitly remain gated.

Current state:

- `POST /api/session/start` with `Live` returns 501 in
  `crates/pm-kalshi/src/web/handlers.rs`.

Implementation options:

- Option A: Keep Live disabled in UI for MVP; ship paper+backtest.
- Option B: Implement a "dry-run live" mode that uses live market data but a
  non-executing executor (no orders), emitting decisions/signals only.
- Option C: Implement real live order execution with Kalshi trading APIs and
  additional safety controls.

Recommended for MVP: Option B, then Option C once observability is proven.

Acceptance Criteria
-------------------

Mode + header

- Switching modes updates header within 1 tick/WS message.
- Attempting to switch while running stops current and starts target without
  server errors.

Paper mode

- Positions/fills/portfolio update live.
- Decisions stream populates DecisionFeed and Timeline.
- Rhythm histogram shows fills over time.

Backtest mode

- Backtest overlay shows progress and completion.
- After completion, fills/equity populate the UI.
- Decisions and timeline are populated (synthetic at minimum).

Failure modes

- If server disconnects, UI falls back to demo data and shows an alert.
- If REST start fails, UI shows the server error string in header and overlay
  (for backtest).

Files Expected To Change (Plan Scope)
------------------------------------

Watchtower:

- `watchtower/src/wire/normalize.ts` (new)
- `watchtower/src/hooks/useWebSocket.ts`
- `watchtower/src/hooks/useGardenStore.ts`
- `watchtower/src/hooks/useModeStore.ts`
- `watchtower/src/hooks/useKeyboardNav.ts`
- `watchtower/src/components/layout/ModeSelector.tsx`
- `watchtower/src/components/layout/Header.tsx` (if needed for authority)

pm-kalshi:

- `crates/pm-kalshi/src/web/ws.rs`
- `crates/pm-kalshi/src/engine/trading.rs`
- (optional) `crates/pm-kalshi/src/web/handlers.rs` for decision endpoints or
  live mode gating.

Testing / Verification Checklist
-------------------------------

- Start pm-kalshi web server on `:3030`.
- Start watchtower with `PM_SERVER_URL=ws://localhost:3030/ws`.
- Enter mode menu, select paper:
  - Header shows `PAPER` and running indicator
  - Positions/Fills update over time
  - Decisions appear (after Phase 4)
- Switch to backtest:
  - Backtest progress overlay shows
  - Completion populates fills/equity/decisions (after Phase 3)
- Switch back to paper:
  - No 409 conflicts
  - Header updates correctly

Implementation Progress
-----------------------

Phase 1: DONE (2026-02-03)

- created `watchtower/src/wire/normalize.ts` with explicit snake_case to
  camelCase functions for all server types: SessionInfo, EngineStatus,
  PortfolioSnapshot, Position, Fill, EquityPoint, CircuitBreaker,
  PipelineMetrics, Bed, Specimen
- created `watchtower/src/wire/index.ts` barrel export
- updated `useWebSocket.ts` to call `normalizeServerMessage()` at ingress
  before passing to handleMessage
- updated `useModeStore.ts` `fetchSessionStatus()` to normalize the REST
  response (trading_active -> tradingActive, session_id -> sessionId, etc.)

Phase 2: DONE (2026-02-03, completed 2026-02-03)

- added `transitionToMode(target: TradingMode)` to useModeStore
  - stops current session if running a different mode
  - sets viewMode then starts session or opens config editor for backtest
- updated `ModeSelector.tsx` to call `transitionToMode` instead of directly
  calling setViewMode + startSession
- updated `useKeyboardNav.ts` enter handler in mode_select to use
  `transitionToMode` instead of duplicated switching logic
- header authority: `useGardenStore.handleMessage` now calls
  `syncSessionToModeStore(msg.session)` on every Welcome, Snapshot, and
  TickUpdate message, overwriting `useModeStore.viewMode` and
  `sessionStatus` with the server's actual session state
- `stopSession()` no longer forcibly sets `viewMode: "idle"` -- it only
  resets `sessionStatus` optimistically; next WS message sets real mode
- replaced 200ms sleep in `transitionToMode` with bounded polling loop:
  polls `GET /api/session/status` up to 10x (100ms apart, ~1s max) waiting
  for `trading_active=false` before starting new session
- fixed backtest config editor: `transitionToMode("backtest")` now calls
  `openConfigEditor()` instead of `setMenuScreen("config_edit")`, so
  `editingConfig` is populated from the active preset

Phase 3: DONE (2026-02-03)

- extended `syncFromBacktest()` in useGardenStore to synthesize decisions
  from trade_log
  - generates "enter" decision for each trade at entry_time
  - generates "exit" decision for each closed trade at exit_time
  - IDs are sequential (`bt-0`, `bt-1`, ...), sorted by timestamp
  - score/confidence set to defaults (0.5/0) since backtest doesn't expose
    per-candidate scorer breakdown
  - reason includes price and pnl info

Phase 4: DONE (2026-02-03)

- added `Decision` variant to `ServerMessage` enum in `ws.rs` with all
  fields (id, timestamp, ticker, action, side, score, confidence,
  scorer_breakdown, reason, fill_id, latency_ms)
- added `DecisionInfo` struct to `engine/trading.rs` and exported from
  `engine/mod.rs`
- added `decisions: Vec<DecisionInfo>` to `TickMetrics`
- engine tick loop now collects decisions:
  - exit fills produce a decision with action="exit"
  - entry fills produce a decision with action="enter"
- main.rs tick forwarder broadcasts each `DecisionInfo` as a
  `ServerMessage::Decision` before broadcasting the TickUpdate

Phase 4 partial: decision persistence DONE (2026-02-03)

- `record_decision()` already existed in `pm_store::SqliteStore` (was
  incorrectly noted as missing earlier). decisions table exists in schema.
- main.rs tick forwarder now converts `DecisionInfo` to `pm_core::Decision`
  and calls `store.record_decision()` before broadcasting over WS
- `DecisionInfo` now carries `timestamp: DateTime<Utc>` from the tick's
  `now` value; main.rs uses `decision.timestamp.to_rfc3339()` instead of
  `Utc::now()`, so decisions are correctly ordered relative to fills
- skip decisions (candidates rejected due to fees/cash/max positions) are
  not emitted yet

Phase 5: NOT STARTED (deferred per plan)

- live mode remains gated (501 from handlers.rs)

Robustness fixes (2026-02-03)

- useWebSocket.ts onmessage: now logs errors and fires an alert instead of
  silently swallowing parse/normalization failures
- normalize.ts: added `asArray()` helper for all `.map()` calls on
  positions, fills, equity_curve, beds, specimens -- prevents crash on
  unexpected null/undefined from server

Files changed
- watchtower/src/wire/normalize.ts (new)
- watchtower/src/wire/index.ts (new)
- watchtower/src/hooks/useWebSocket.ts
- watchtower/src/hooks/useGardenStore.ts
- watchtower/src/hooks/useModeStore.ts
- watchtower/src/hooks/useKeyboardNav.ts
- watchtower/src/components/layout/ModeSelector.tsx
- crates/pm-kalshi/src/web/ws.rs
- crates/pm-kalshi/src/engine/trading.rs
- crates/pm-kalshi/src/engine/mod.rs
- crates/pm-kalshi/src/main.rs

Build status: rust compiles, typescript passes type-check

Remaining known gaps
- skip decisions not emitted (candidates rejected by circuit breaker,
  cash reserve, max positions, or fee limits)
- equity computations use avg_entry_price not mark-to-market for
  positions value (acceptable for MVP)
- live mode (Phase 5) deferred
