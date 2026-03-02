# prediction-markets monorepo task runner

# list available recipes
default:
    @just --list

# === garden commands ===

# start the greenhouse (pm-server)
greenhouse:
    cargo run -p pm-server

# harvest the crops (show positions)
harvest:
    cargo run -p pm-server -- --show-positions

# prune dormant specimens
prune:
    cargo run -p pm-server -- --prune-dormant

# === kalshi ===

# run kalshi paper trading
kalshi-paper:
    cargo run --release -p pm-kalshi -- paper --config config.toml

# run kalshi paper trading + watchtower together (one command)
kalshi-dev:
    @bash -lc 'set -euo pipefail; trap "kill 0" EXIT INT TERM; \
      cargo run --release -p pm-kalshi -- paper --config config.toml & \
      PM_SERVER_URL=ws://127.0.0.1:3030/ws bun --cwd watchtower dev & \
      wait'

# run kalshi backtest
kalshi-backtest:
    cargo run --release -p pm-kalshi -- run --data-dir data --start 2024-01-01 --end 2024-06-01 --capital 10000

# ingest kalshi CSV data into sqlite
kalshi-ingest:
    cargo run --release -p pm-kalshi -- ingest --data-dir data --db data/historical.db

# run kalshi backtest from sqlite (fast)
kalshi-backtest-fast:
    cargo run --release -p pm-kalshi -- run --db data/historical.db --start 2024-01-01 --end 2024-06-01 --capital 10000

# build kalshi trader
kalshi-build:
    cargo build --release -p pm-kalshi

# run kalshi tests
kalshi-test:
    cargo test -p pm-kalshi

# === polymarket (not yet ported) ===

# run polymarket paper trading
poly-paper:
    @echo "polymarket engine not yet ported — see compost/README.md"

# fetch polymarket weather markets
poly-markets:
    @echo "polymarket engine not yet ported — see compost/README.md"

# test NWS weather API
poly-weather:
    @echo "polymarket engine not yet ported — see compost/README.md"

# sync polymarket dependencies
poly-sync:
    @echo "polymarket engine not yet ported — see compost/README.md"

# run polymarket tests
poly-test:
    @echo "polymarket engine not yet ported — see compost/README.md"

# === scripts ===

# fetch fresh kalshi data
fetch-kalshi:
    uv run tools/fetch_kalshi_data_v2.py

# === data management ===

# show data directory status
data-status:
    @echo "kalshi data:"
    @ls -lh data/ 2>/dev/null || echo "  (no data)"

# === js packages ===

# start watchtower TUI
watchtower:
    cd watchtower && bun dev

# start web dev server
web:
    cd web && bun dev

# build all JS packages
js-build:
    bun run build

# typecheck all JS packages
js-check:
    bun run typecheck
