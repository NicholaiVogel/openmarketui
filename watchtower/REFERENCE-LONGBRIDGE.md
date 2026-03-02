# Reference: longbridge-terminal

Feature and architecture reference from Longbridge's trading TUI.

**Repository:** https://github.com/longbridge/longbridge-terminal  
**Demo:** https://asciinema.org/a/785102  
**Added:** 2026-02-15

---

## Overview

Rust-based TUI stock trading terminal. Different stack (Rust/Ratatui vs our 
TypeScript/OpenTUI) but solid feature and architecture ideas to draw from.

## Their Stack

| Component | Longbridge | Watchtower |
|-----------|------------|------------|
| Language | Rust | TypeScript |
| TUI Framework | Ratatui | OpenTUI (React) |
| State Management | Bevy ECS | React hooks |
| Async | Tokio | Bun/Node |
| Data Transport | WebSocket | WebSocket |

---

## Features Worth Grabbing

### 1. Candlestick Charts
They have a dedicated crate: `crates/cli-candlestick-chart`

Could adapt the rendering logic for terminal output - useful for:
- Position visualization
- Historical yield charts
- Strategy performance over time

### 2. Real-time Watchlist
- Live market data push via WebSocket
- Efficient diff-based updates
- Color-coded price changes (green/red)

### 3. Stock Search with Autocomplete
- Fuzzy search component
- Keyboard navigation (j/k or arrows)
- Quick symbol lookup

### 4. Vim-like Keybindings
Their bindings are intuitive for terminal users:
- `j/k` for up/down navigation
- `/` for search
- Number keys for tab switching (we already do this)
- `q` to quit

### 5. Multi-view Layout
- Watchlist view
- Portfolio view  
- Individual stock detail view
- Clean tab-based navigation

### 6. Popup/Modal System
- Confirmation dialogs
- Detail overlays
- Search popups

---

## Architecture Patterns

### Data Flow
```
WebSocket push
    ↓
Parse event → Update global cache (DashMap)
    ↓
UI reads from cache → Render
```

Similar to our pattern but they use:
- `OnceLock` for global singleton contexts
- `DashMap` for thread-safe cache
- Bevy ECS systems for render scheduling

### Module Structure
```
src/
├── openapi/          # API integration layer
│   └── context.rs    # Global contexts (QuoteContext, TradeContext)
├── data/             # Data types and cache
│   ├── types.rs      # Counter, QuoteData, Candlestick
│   ├── stock.rs      # Stock struct
│   └── stocks.rs     # Global STOCKS cache
├── api/              # API call wrappers
│   ├── search.rs
│   ├── quote.rs
│   └── account.rs
├── widgets/          # UI components
├── views/            # Page views
├── app.rs            # Main loop
└── system.rs         # Render logic
```

### Key Patterns

**Global State via Singleton**
```rust
// They use OnceLock for global contexts
static QUOTE_CTX: OnceLock<QuoteContext> = OnceLock::new();
pub fn quote() -> &'static QuoteContext { ... }
```

**Cache with DashMap**
```rust
// Thread-safe concurrent hashmap
static STOCKS: Lazy<DashMap<String, Stock>> = Lazy::new(DashMap::new);
```

**Update Methods on Data Structs**
```rust
impl Stock {
    pub fn update_from_quote(&mut self, quote: &PushQuote) { ... }
    pub fn update_from_depth(&mut self, depth: &PushDepth) { ... }
}
```

---

## Potential Adaptations for Watchtower

### Candlestick Component
Could create an OpenTUI component that renders ASCII candlesticks:
```
     │
   ┌─┴─┐
   │   │  <- body (filled = red, hollow = green)
   └─┬─┘
     │
```

### Enhanced Keybindings
Consider adding:
- `/` to trigger search/filter mode
- `?` for help overlay
- `g` prefix for "go to" commands (gg = top, G = bottom)

### Search Component
Add fuzzy search for:
- Specimens by name
- Beds by family
- Historical harvests

### Loading States
They have a nice `Loading` widget with animation - could improve UX during 
reconnection or data fetch.

---

## Resources

- Rust SDK Docs: https://longportapp.github.io/openapi/rust/longport/
- OpenAPI Docs: https://open.longbridge.com
- Their CLAUDE.md: https://github.com/longbridge/longbridge-terminal/blob/main/CLAUDE.md
