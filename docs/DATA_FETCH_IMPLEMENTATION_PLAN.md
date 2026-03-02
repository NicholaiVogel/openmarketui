Data Fetching Implementation Plan
===

Phase 1: Data Manager UI Fixes
1.1 Fix j/k navigation in DataManager
- Add menuScreen === "data_manager" case to j/k handlers in useKeyboardNav.ts
- Wire up to call moveDateRangeIndex(1) or moveDateRangeIndex(-1)
1.2 Add comprehensive UI controls
Add these inputs to DataManager component:
| Control | Type | Presets | Default |
|---------|-------|----------|----------|
| Trades per day | number input | 1K, 10K, 50K, 100K | 10,000 |
| Time range preset | select | Last 7, 30, 60, 90 days, 6 months, 1 year | Last 30 days |
| Custom start date | date picker (manual entry) | - | - |
| Custom end date | date picker (manual entry) | - | - |
Add trade count estimate display:
- Calculate: days × trades_per_day = estimated_trades
- Show: "~X trades (~Y MB)" to give visibility into data size
1.3 Add data location configuration
Create new settings section accessible from main menu:
- Path input with validation
- Default: /mnt/work/kalshi-data (existing symlink)
- Save to watchtower/.config/settings.json or similar
Phase 2: Rust DataFetcher Enhancement
2.1 Add market metadata fetching
Implement new method fetch_markets_incremental():
async fn fetch_markets_incremental(
    &self,
    state: Arc<RwLock<FetchState>>,
) -> anyhow::Result<usize> {
    // GET /markets?limit=1000
    // Follow cursor pagination until no more
    // Append to markets.csv with all fields
    // Update state tracking
}
Markets.csv fields (matching Python script):
- ticker, title, category
- open_time, close_time
- result (yes/no/cancelled)
- status (open/closed/cancelled)
- yes_bid, yes_ask
- volume, open_interest
2.2 Extend FetchStateFile
pub struct FetchStateFile {
    // Existing trades state
    completed_days: Vec<String>,
    current_day: Option<String>,
    current_day_cursor: Option<String>,
    current_day_count: usize,
    total_trades: usize,
    
    // NEW: Markets state
    markets_cursor: Option<String>,
    markets_count: usize,
    markets_done: bool,
}
2.3 Implement append-only mode
For both trades and markets:
- Check if file exists before writing
- If exists, open in append mode, skip header row
- Only write new data (don't overwrite)
- Validate no duplicate entries (by timestamp+ticker for trades, by ticker for markets)
2.4 Smart fetch coordination
pub async fn fetch_range(&self, ...) -> anyhow::Result<PathBuf> {
    // 1. Fetch markets FIRST (smaller, provides context)
    if !state.markets_done {
        self.fetch_markets_incremental(state.clone()).await?;
    }
    
    // 2. Then fetch trades
    if !state.trades_done {
        self.fetch_trades_incremental(...).await?;
    }
}
Phase 3: API Layer Updates
3.1 Extend DataFetchRequest
pub struct DataFetchRequest {
    pub start_date: String,
    pub end_date: String,
    pub trades_per_day: usize,          // NEW: default 10000
    pub fetch_markets: bool,             // NEW: default true
    pub fetch_trades: bool,              // NEW: default true
    pub data_dir: Option<String>,         // NEW: override default
}
3.2 Enhanced DataAvailability
pub struct DataAvailability {
    pub has_data: bool,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    
    // ENHANCED
    pub total_trades: usize,
    pub total_markets: usize,           // NEW
    pub days_count: usize,
    
    // COMPLETENESS CHECKS
    pub has_markets: bool,             // NEW: markets.csv exists?
    pub has_trades: bool,             // NEW: trades.csv exists?
    pub is_complete: bool,              // NEW: both files present?
}
Backend implementation:
pub async fn get_available_data(...) -> DataAvailability {
    let markets_path = data_dir.join("markets.csv");
    let trades_path = data_dir.join("trades.csv");
    
    DataAvailability {
        has_trades: trades_path.exists(),
        has_markets: markets_path.exists(),
        is_complete: markets_path.exists() && trades_path.exists(),
        // ... scan both files for counts/ranges
    }
}
3.3 Add data directory config endpoint
pub async fn post_data_config(
    State(state): State<Arc<AppState>>,
    Json(req): DataConfigRequest,
) -> StatusCode {
    // Validate path exists
    // Update state.config.data_dir
    // Return new config
}
Phase 4: Frontend Updates
4.1 Enhanced DataManager component
State management:
const [tradesPerDay, setTradesPerDay] = useState(10000);
const [usePresets, setUsePresets] = useState(true);
const [customStart, setCustomStart] = useState("");
const [customEnd, setCustomEnd] = useState("");
const [tradesPresetIndex, setTradesPresetIndex] = useState(1); // 10K default
Trades per day presets UI:
trades/day: [^ v]
presets: [1K] [10K] [50K] [100K]
Keyboard navigation:
- When using presets: j/k to select preset
- When using custom: just display selected value
- Toggle between preset/custom mode with key
4.2 Settings screen for data location
Add to main menu (under CommandMenu ROOT_MENU_ITEMS):
- New item: { id: "settings", label: "Settings", hint: "configure watchtower" }
Settings screen shows:
- Current data directory path
- Edit path option
- Validate path on save (check writeable, exists)
4.3 Updated status display
When data available, show:
available: 2024-01-01 to 2024-06-30
trades: 1.2M (182 days)
markets: 15.3K
When incomplete, show warnings:
⚠ missing markets.csv - backtest won't work!
Phase 5: Testing Strategy
5.1 Unit tests for data fetching
- Test market CSV generation matches expected format
- Test append mode doesn't duplicate entries
- Test state persistence across restarts
- Test cursor pagination through full market list
5.2 Integration tests with backtest
- Fetch small dataset (1 day, 1K trades)
- Run backtest → verify candidates found
- Verify zero-trades issue is resolved
5.3 Manual testing scenarios
| Scenario | Expected Result |
|----------|-----------------|
| Fresh fetch (no existing data) | Creates both files, shows complete status |
| Resume after cancel | Continues from last saved cursor |
| Append to existing data | Only new days added, old data intact |
| Overlap date range | Skips existing days, fetches only new |
| Missing markets.csv | Shows warning, backtest blocked with helpful message |
| Change data directory | New path used, old data preserved |
---
Implementation Order Recommendation
Priority 1 (Critical path to fix zero-trades):
1. Fix j/k navigation (quick, blocking)
2. Add markets fetching to Rust fetcher
3. Update backtest to use markets.csv
4. Verify zero-trades resolved
Priority 2 (UX improvements):
5. Add trades per day configuration
6. Add date range presets
7. Implement append-only mode
Priority 3 (Nice-to-have):
8. Data directory configuration
9. Enhanced UI polish
10. Additional metadata fields
---
Questions Before Execution
1. Trade count presets values: Are 1K, 10K, 50K, 100K good? Should we add 500 or 250K?
2. Date preset granularity: Do we want "Last 7 days" and "Last 14 days" or is 30 days the smallest useful window?
3. Data validation: Should we validate fetched data integrity (check for duplicate timestamps, malformed entries) and show warnings?
4. Error handling: If markets fetch succeeds but trades fails (or vice versa), should we:
   - Keep the successful file and retry the failed one?
   - Delete both and force full re-fetch?
