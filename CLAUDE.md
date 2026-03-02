# CLAUDE.md

This file provides guidance to AI code tools like Claude Code (claude.ai/code) or Opencode (opencode.ai) when working with code in this repository.

openmarketui
===

interface for building heuristic algorithms, training custom trading models, and deploying strategies on prediction markets — all from a user-friendly, performant UI. the rust backend handles execution and backtesting; the web and watchtower layers are the product surface users interact with.

strategies are "specimens" organized into "beds", trades are "harvests", filters are the "immune system".

build commands
---

```bash
# rust workspace
cargo build                    # build all rust crates
cargo test                     # run all rust tests
cargo run -p pm-server         # start the server

# kalshi backtesting
just kalshi-backtest           # or: cargo run --release -p pm-kalshi -- run --data-dir data --start 2024-01-01 --end 2024-06-01 --capital 10000

# kalshi paper trading (requires config.toml)
just kalshi-paper              # or: cargo run --release -p pm-kalshi -- paper --config config.toml

# js packages (bun workspaces + turbo)
bun install                    # install all js deps (hoisted to root)
bun run dev                    # start watchtower + web in parallel
bun run watchtower             # start watchtower TUI only
bun run web                    # start web dev server only
bun run build                  # build all js packages
bun run typecheck              # typecheck all js packages

# task runner (just)
just                           # list all tasks
just greenhouse                # start pm-server
just kalshi-backtest           # run kalshi backtest
just watchtower                # start watchtower TUI
just web                       # start web dev server
just poly-test                 # (stub — polymarket not yet ported)
```

architecture
---

```
crates/
├── pm-core       # foundational types and traits (MarketCandidate, TradingContext)
├── pm-store      # sqlite persistence layer ("root cellar")
├── pm-garden     # scorers/filters organized into beds ("the garden")
├── pm-engine     # risk management, execution, backtesting engine
├── pm-server     # REST + WebSocket server ("greenhouse")
└── pm-kalshi     # kalshi trading engine (binary + lib)

compost/          # standalone trading engines (not in workspace)
├── kalshi/       # planned: pm-kalshi will migrate here when stable
└── polymarket/   # planned: python trader for polymarket weather markets

watchtower/       # react-based terminal UI (opentui) for monitoring
web/              # astro landing page (cloudflare pages)
tools/            # python scripts for data fetching
```

js workspace
---

bun workspaces with turbo for build orchestration:

- `watchtower/` — package `openmarketui-watchtower` (opentui + react + zustand)
- `web/` — package `openmarketui-web` (astro + tailwind + cloudflare pages)
- root `package.json` manages workspaces, `turbo.json` manages build pipeline

pipeline architecture
---

data flows through a trait-based pipeline:

`Source` → `Filter` → `Scorer` → `Selector` → `OrderExecutor`

- **Source** (`pm_core::Source`): fetches market candidates (live API or CSV)
- **Filter** (`pm_core::Filter`): removes unsuitable candidates (liquidity, spread, time constraints)
- **Scorer** (`pm_core::Scorer`): evaluates candidates and writes scores to `candidate.scores` HashMap
- **Selector** (`pm_core::Selector`): picks which scored candidates to trade
- **OrderExecutor** (`pm_core::OrderExecutor`): executes signals (backtest fills vs paper/live orders)

all pipeline traits are in `crates/pm-core/src/traits.rs`.

key types
---

- `MarketCandidate`: the primary data structure flowing through the pipeline (ticker, prices, volume, scores)
- `TradingContext`: current state passed through pipeline (timestamp, portfolio, trading history)
- `PipelineBuilder`: fluent builder for constructing trading pipelines
- `TradingPipeline`: orchestrates the full Source→Filter→Scorer→Selector→Executor flow

garden beds (scorer families)
---

scorers in `pm-garden` are organized by strategy type:

- `beds/kalshi/momentum.rs` - trend following (MomentumScorer, TimeDecayScorer)
- `beds/kalshi/mean_reversion.rs` - mean reversion (BollingerMeanReversionScorer)
- `beds/kalshi/volume.rs` - flow analysis (VolumeScorer, VPINScorer, OrderFlowScorer)
- `beds/kalshi/ensemble.rs` - combination strategies (WeightedScorer, BayesianEnsembleScorer)

filters in `filters/mod.rs`: LiquidityFilter, SpreadFilter, PriceRangeFilter, TimeToCloseFilter, etc.

kalshi engine specifics
---

the kalshi engine lives in `crates/pm-kalshi/` (workspace member, planned to migrate to `compost/kalshi/`):

- `api/client.rs` - kalshi API client (2 req/sec rate limit for markets endpoint)
- `sources/paper_executor.rs` - simulated order execution for paper trading
- `engine/` - trading logic and state
- `backtest.rs` - backtesting orchestration
- `web/` - axum handlers for web dashboard

paper trading requires a `config.toml` at repo root — see `fertilizer/kalshi/config.toml.example` for template (not yet created; current working config is `config.toml`).

polymarket engine specifics
---

python-based trader — not yet ported to this repo. see `compost/README.md`.

planned location `compost/polymarket/`:

- uses `py-clob-client` for polymarket API
- integrates NWS/NOAA weather data for edge
- separate pipeline implementation in `poly/pipeline/`

# currentDate
Today's date is 2026-02-26.

      IMPORTANT: this context may or may not be relevant to your tasks. You should not respond to this context unless it is highly relevant to your task.
