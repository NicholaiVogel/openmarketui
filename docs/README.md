# OpenMarketUI Documentation

Interface for building heuristic algorithms, training custom trading models, and deploying strategies on prediction markets. The Rust backend handles execution and backtesting; the web and watchtower layers are the product surface.

---

## How this documentation is organized

Following the [Diátaxis](https://diataxis.fr/) framework, docs are split into four types:

**Tutorials** — Learning-oriented. Start here if you're new to the codebase.

- [Your First Backtest](tutorials/01-first-backtest.md)
- [Building a Custom Scorer](tutorials/02-building-a-custom-scorer.md)
- [Setting Up Paper Trading](tutorials/03-setting-up-paper-trading.md)

**How-to Guides** — Problem-oriented recipes for specific tasks.

- [Configure Exit Rules](how-to/configure-exit-rules.md)
- [Add a Filter to the Immune System](how-to/add-a-filter.md)
- [Ingest Historical Data](how-to/ingest-historical-data.md)
- [Monitor a Session in Watchtower](how-to/monitor-with-watchtower.md)
- [Tune the Circuit Breaker](how-to/tune-circuit-breaker.md)
- [Deploy the Landing Page](how-to/deploy-web.md)

**Reference** — Technical descriptions of the system's machinery.

- [Pipeline Traits API](reference/pipeline-traits.md)
- [Core Types](reference/core-types.md)
- [config.toml Reference](reference/config-toml.md)
- [CLI Reference (pm-kalshi)](reference/cli-reference.md)
- [Scorers and Filters Catalog](reference/scorers-and-filters.md)
- [Watchtower UI Reference](reference/watchtower-ui.md)

**Explanation** — Conceptual background for understanding the system.

- [The Garden Metaphor](explanation/garden-metaphor.md)
- [Pipeline Architecture and Data Flow](explanation/pipeline-architecture.md)
- [Position Sizing: Kelly Criterion and Risk Controls](explanation/position-sizing.md)
- [Backtesting Methodology](explanation/backtesting-methodology.md)

---

## Quick orientation

The domain language uses a garden metaphor throughout — see [The Garden Metaphor](explanation/garden-metaphor.md) for why. Here's the short version:

| Domain term | Conventional term |
|---|---|
| Specimen | Strategy / scorer |
| Bed | Strategy family |
| Harvest | Trade / fill |
| Yield | P&L |
| Immune system | Filter set |
| Greenhouse | Trading server |

The pipeline follows a fixed shape: `Source → Filter → Scorer → Selector → OrderExecutor`. Every component is a trait — you can swap any stage independently.
