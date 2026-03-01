# OpenMarketUI Interface System

## Direction and Feel
- Product intent: terminal-native trading cockpit with dense but calm information hierarchy.
- Visual tone: restrained, monochrome-first structure with semantic accents only for state (success/warning/error).
- Interaction posture: read-fast, act-fast, low ornament, strong scanability.

## Depth Strategy
- Primary depth approach: borders + subtle surface contrast (no heavy shadows).
- Panel separation: single-line border using shared border token.
- Selection/focus emphasis: background shift to `bgAlt`, not stronger borders.
- Data emphasis: color is reserved for meaning (PnL direction, active/paused, execution state).

## Spacing System
- Base unit: 1 terminal cell.
- Typical component spacing:
  - In-panel horizontal padding: 1
  - Section gap inside panels: 1
  - Inter-panel vertical gap: 1
  - Dense row tables: gap 0, fixed-width columns for alignment

## Overview Pattern (Saved)

### Layout
- Overview uses two columns:
  - Left: `session` control strip (fixed narrow width).
  - Right: stacked panels:
    - `strategies` table (top)
    - `market pulse` (bottom, flex-grow)

### Strategies Table
- Columns: icon, name, group, status, weight, distribution.
- Distribution rendered as compact text bar using block/soft block chars.
- Keep high-density row format with minimal vertical whitespace.

### Market Pulse Panel
- Split into two regions:
  - Left (chart region): equity spark chart with `high/low` labels and top-line delta (`$` + `%`).
  - Right (tape region): recent trades list with `price`, `size`, `time ago`.
- Chart style:
  - Width target: ~52 chars
  - Height target: ~10 rows
  - Filled blocks for active area, light blocks for background grid.
- Tape style:
  - Width target: ~30 chars
  - Rows: most recent first (up to ~16)
  - Size formatting: raw -> K -> M compaction.

### Data Source Contract
- Chart source: `equityCurve[].equity`.
- Tape source: `recentFills[]`.
- Direction color:
  - Up/effective positive: `success`
  - Down/effective negative: `error`
  - Neutral metadata: `textDim`

## Reuse Rules
- Reuse `Panel` as the outer structure for all overview modules.
- Preserve fixed-width tabular columns for any tape/list element.
- New overview widgets must fit the same two-column rhythm and 1-cell spacing scale.
