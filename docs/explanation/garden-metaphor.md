# The Garden Metaphor

OpenMarketUI uses a consistent garden metaphor throughout the codebase, documentation, and UI. This document explains the mapping and the reasoning behind it.

## The mapping

| Garden term | Trading term | Where it appears |
|---|---|---|
| Specimen | Individual scorer / strategy | `pm-garden`, Watchtower UI, WebSocket messages |
| Bed | Family of related scorers | `beds/kalshi/`, `beds/advanced/` |
| Greenhouse | Trading server | `pm-server`, `just greenhouse` task |
| Harvest | A trade fill | `watchtower/src/tabs/CurrentHarvest.tsx` |
| Yield | P&L | WebSocket `YieldUpdate` message |
| Immune system | Filter set | `pm-garden/src/filters/` |
| Root cellar | SQLite persistence | `pm-store` crate |
| Watering | Fetching market candidates | Trait comment in `pm-core/src/traits.rs` |
| Bloom / dormant | Enabled / disabled specimen | Watchtower specimen state |
| Prune | Exit a position | `crates/pm-core/src/exit.rs` |

## Why a metaphor

Prediction markets share a structural property with agriculture: you plant positions, tend them over time, and harvest outcomes. The metaphor isn't just decorative — it shapes how developers reason about the system.

**Specimens and beds** make it natural to think about scorers as living things with a lifecycle (bloom, dormant) and a habitat (the bed they belong to). A `MomentumScorer` belongs to the `kalshi/momentum` bed the way a plant belongs to a botanical family. When you add a new scorer, you're planting a new specimen.

**The immune system** for filters maps well conceptually. Filters don't evaluate whether a market is *worth trading* — they reject candidates that are fundamentally unsuitable. This is exactly how an immune system works: it doesn't optimize for the best outcomes, it eliminates threats before they reach the decision layer. A `LiquidityFilter` isn't ranking markets by liquidity; it's quarantining anything below a minimum threshold.

**Harvest vs. yield** distinguishes individual fills (harvests) from cumulative P&L (yield). You can have many harvests and still have negative yield if the timing was wrong. This distinction matters when building dashboards — showing harvest counts without yield is misleading, and showing yield without harvest activity obscures how it was generated.

## Consistency rules

When extending the system, keep the metaphor consistent:

- New scorer types belong to a **bed**. If no existing bed fits, create one under `beds/`.
- Filters are part of the **immune system**, not the bed structure. Don't put filters in a bed directory.
- Exit signals are **pruning** operations. The metaphor: you prune a specimen when it's no longer healthy.
- Positions held in the portfolio are things **in the ground** — planted but not yet harvested.
- The WebSocket layer broadcasts **garden updates** to observers (watchtower). Use `GardenUpdate`, `SpecimenUpdate`, `HarvestUpdate`, `YieldUpdate` for new message types.

## Where the metaphor breaks down

The metaphor doesn't extend perfectly everywhere. A few places where the mapping is loose:

- `pm-engine` contains risk management (circuit breaker) and position sizing (Kelly criterion). These don't have a clean garden analog — they're financial engineering, not horticulture. The code doesn't force a metaphor here.
- The `OrderExecutor` trait is called the "harvester" in comments, but in code it's `OrderExecutor`. The domain metaphor lives in documentation and UI, not in type names where clarity matters more.
- `pm-store` is called the "root cellar" in project docs, but the crate is named `pm-store`. Same principle: metaphor for human communication, descriptive names for code.
