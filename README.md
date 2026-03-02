OpenmarketUI
===

Interface for building heuristic algorithms, training custom models, and deploying trading strategies on prediction markets.

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
just kalshi-dev                # start kalshi paper + watchtower together

# js packages (bun workspaces + turbo)
bun install                    # install all js deps (hoisted to root)
bun run kalshi-dev             # start kalshi paper + watchtower together
bun run dev                    # start watchtower + web in parallel
bun run watchtower             # start watchtower TUI only
bun run web                    # start web dev server only
bun run build                  # build all js packages
bun run typecheck              # typecheck all js packages

# task runner (just)
just                           # list all tasks
just greenhouse                # start pm-server
just kalshi-dev                # start kalshi paper + watchtower together
just kalshi-backtest           # run kalshi backtest
just watchtower                # start watchtower TUI
just web                       # start web dev server
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
├── kalshi/       # planned: pm-kalshi migrates here when stable
└── polymarket/   # planned: python trader for polymarket weather markets

watchtower/       # react-based terminal UI (opentui) for monitoring
web/              # astro landing page (cloudflare pages)
tools/            # python scripts for data fetching
```

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

the kalshi engine lives in `crates/pm-kalshi/` (planned to migrate to `compost/kalshi/`):

- `api/client.rs` - kalshi API client (2 req/sec rate limit for markets endpoint)
- `sources/paper_executor.rs` - simulated order execution for paper trading
- `engine/` - trading logic and state
- `backtest.rs` - backtesting orchestration
- `web/` - axum handlers for web dashboard

paper trading requires a `config.toml` at repo root.

polymarket engine specifics
---

python-based trader — not yet ported. see `compost/README.md` for migration plan.
