# Sharp Book Line Movement Capture & Analysis Pipeline

## Purpose

Extract actionable sharp signals from Pinnacle, Circa, and Bookmaker line movements using aggregator data. No direct API access exists to these books — the line movement *is* the signal. This pipeline captures, stores, analyzes, and broadcasts sharp action events derived purely from line data.

## Theory

In sports betting and EV (expected value) betting, the fundamental goal is finding mispriced trades—identifying numbers that are better than what the broader market suggests they should be. This approach, called top-down trading, treats the market itself as the source of truth. When you see a sharp sportsbook move their line and other books follow, that's your signal to copy the trade at books that haven't adjusted yet. For example, if Kalshi, Polymarket, and Circa all move a line up by 1.5 points, you can replicate that trade at slower books before they catch up.

The challenge we're solving is adapting this proven sports betting strategy to prediction markets. Market makers on Kalshi and Polymarket often copy their pricing from traditional sports betting principles or mirror the "big dogs"—established sharp books like Circa, Bookmaker, and Pinnacle. These three books are the market leaders. They don't just move first; they set the price that everyone else follows. A "sharp" is a bettor who wins consistently over time, and sharp books are the ones that take action from these winning bettors.

Here's the key insight: Kalshi and Polymarket by themselves aren't worth trying to game directly. What matters is that sharp books take sharp action, and how they move determines how the rest of the market moves. By copying their moves to slower books, you can capture profits. But there's a twist—the people making markets on Kalshi are often just copying those same sharp books. So if Circa, Pinnacle, and Bookmaker all move in the same direction, we can build a system to identify the exact sources of those moves on prediction markets.

Our approach uses filtering and tracking to create a vetting system that guarantees trades are worth following. The primary method is latency fingerprinting: we identify traders by tracking their timing relative to other market participants. We use their position in time—how quickly they react compared to others—as a way to identify them without needing to know their actual identity.

We treat Pinnacle, Circa, Bookmaker, Kalshi, and Polymarket as our sources of truth. The fingerprinting system identifies which traders consistently align with sharp book pricing before the rest of the market catches up. We then tail those specific traders with position sizing determined by our confidence in their identity and their historical edge. We don't need to know who placed a trade—we just need to observe it with enough temporal resolution to see the pattern.

Polymarket provides rich data through the CLOB API, which offers real-time WebSocket streams of trades and order book changes as they happen. Their Activity and Market Data API lets us track everything by wallet ID, while the Gamma API provides market metadata, volume, categorization, and historical price data. For real-time trade data, the CLOB API gives us the tools necessary to replicate a full trading interface. We use Polymarket and Kalshi for behavioral fingerprinting because they offer rich metadata, while we use Circa, Bookmaker, and Pinnacle to map broader sharp movement patterns.

The critical observation is that Pinnacle, Circa, and Bookmaker don't all move at the same time. When sharp action hits, one of them typically moves first. By capturing all three from an aggregator at high frequency, we can observe which book leads on any given market. The system we need pulls from the aggregator at the highest possible frequency, takes snapshots with timestamps, runs analysis on the differences between consecutive snapshots, and outputs a sharp action event log derived entirely from line movement patterns.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    DATA INGESTION                        │
│                                                         │
│  ┌───────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  OpticOdds    │  │  Unabated    │  │  Backup:     │  │
│  │  SSE Stream   │  │  WebSocket   │  │  The Odds    │  │
│  │  (primary)    │  │  (secondary) │  │  API (poll)  │  │
│  └──────┬────────┘  └──────┬───────┘  └──────┬───────┘  │
│         │                  │                 │           │
│         └──────────┬───────┴─────────────────┘           │
│                    ▼                                     │
│         ┌──────────────────┐                             │
│         │  Ingestion       │                             │
│         │  Normalizer      │                             │
│         │  (unified schema)│                             │
│         └────────┬─────────┘                             │
└──────────────────┼──────────────────────────────────────┘
                   ▼
┌─────────────────────────────────────────────────────────┐
│                   LINE STORE (TimescaleDB)               │
│                                                         │
│  line_snapshots: every captured price point per book     │
│  line_diffs: computed deltas between consecutive snaps   │
│  event_metadata: fixture/market/sport identifiers        │
└──────────────────┬──────────────────────────────────────┘
                   ▼
┌─────────────────────────────────────────────────────────┐
│                 ANALYSIS ENGINE                          │
│                                                         │
│  ┌────────────────┐  ┌─────────────────┐                │
│  │  Move Detector │  │  Steam Detector │                │
│  │  (per-book     │  │  (cross-book    │                │
│  │   line deltas) │  │   correlation)  │                │
│  └───────┬────────┘  └────────┬────────┘                │
│          │                    │                          │
│  ┌───────▼────────┐  ┌───────▼──────────┐               │
│  │  Origination   │  │  RLM Detector    │               │
│  │  Tracker       │  │  (reverse line   │               │
│  │  (who moved    │  │   movement vs    │               │
│  │   first?)      │  │   public %)      │               │
│  └───────┬────────┘  └────────┬─────────┘               │
│          │                    │                          │
│          └────────┬───────────┘                          │
│                   ▼                                      │
│         ┌──────────────────┐                             │
│         │  Sharp Action    │                             │
│         │  Event Emitter   │                             │
│         └────────┬─────────┘                             │
└──────────────────┼──────────────────────────────────────┘
                   ▼
┌─────────────────────────────────────────────────────────┐
│              SHARP ACTION EVENT BUS                      │
│                                                         │
│  → Polymarket fingerprinting system (Layer 2)           │
│  → Kalshi behavioral clustering system                  │
│  → Cross-market arbitrage scanner                       │
│  → Alerts / logging                                     │
└─────────────────────────────────────────────────────────┘
```

---

## Layer 1: Data Ingestion

### Aggregator Selection

**Primary: OpticOdds**
- SSE streaming endpoint for real-time odds updates
- Covers Pinnacle, Circa Sports, and Bookmaker.eu
- `copilot-odds` event type pushes line changes as they happen
- RabbitMQ option available for high-volume consumption
- Rate limit: 2500 req/15sec (non-streaming), 250 new SSE connections/15sec
- Pricing: Tiered, need at minimum the plan that covers all three sharp books

**Secondary: Unabated**
- WebSocket feed covering 25+ sportsbooks including Pinnacle and Circa
- Good for cross-validation — if OpticOdds reports a move, does Unabated confirm it?
- Also useful as a fallback if OpticOdds has an outage

**Backup/Historical: The Odds API**
- REST polling at 5-10 minute intervals
- Too slow for real-time sharp detection but useful for historical backtesting
- Cheaper tier covers all three books
- Good for building the initial dataset to train/tune analysis thresholds

### Normalized Schema

Every line update from any aggregator gets normalized into a single format before storage:

```
LineSnapshot {
    id:                 UUID
    source:             enum(opticodds, unabated, theoddsapi)
    book:               enum(pinnacle, circa, bookmaker)
    sport:              string        // e.g., "nfl", "nba", "mlb"
    fixture_id:         string        // normalized event identifier
    fixture_name:       string        // human-readable, e.g., "LAL @ BOS"
    market_type:        string        // "spread", "total", "moneyline", "prop"
    market_key:         string        // specific market identifier
    selection:          string        // "home", "away", "over", "under"
    price:              float         // American odds or decimal
    price_decimal:      float         // always store decimal for math
    line:               float|null    // spread/total number, null for ML
    captured_at:        timestamp(ms) // when our system received it
    source_timestamp:   timestamp|null // aggregator-reported timestamp if available
    previous_price:     float|null    // last known price for this exact market+book+selection
    previous_line:      float|null    // last known line
}
```

**Key design decisions:**
- Store both American and decimal odds. American is what bettors see, decimal is what you calculate with.
- `captured_at` vs `source_timestamp`: The aggregator might report when *it* saw the change, but `captured_at` is when *your system* ingested it. The delta between these tells you your own latency.
- `previous_price` and `previous_line` are denormalized for fast diff computation without lookups.
- `fixture_id` normalization is critical — OpticOdds and Unabated may use different IDs for the same game. Need a mapping layer.

### Fixture ID Resolution

This is one of the harder problems. Different aggregators use different identifiers for the same event. You need a resolution layer that maps:

```
OpticOdds fixture_id "OO-NFL-2026-W05-LAL-BOS" 
Unabated fixture_id  "UNB-12345"
The Odds API event_id "abc123def456"
→ Internal canonical_fixture_id "fix_nfl_20260204_lal_bos"
```

Approaches:
- **Team name + date matching**: Most reliable for major sports. Parse team names and game dates from each source, match on normalized versions.
- **Start time matching**: If two sources report the same sport, same teams (fuzzy match), same start time within 5 minutes, it's the same game.
- **Manual override table**: For edge cases (e.g., doubleheaders, rescheduled games), maintain a manual mapping.
- **Cross-reference via a third source**: Use a canonical sports data source (ESPN API, or similar) as ground truth for fixture identity.

### Ingestion Workers

Run one worker per aggregator source:

**OpticOdds SSE Worker:**
```
1. Connect to SSE endpoint with auth headers
2. Filter for copilot-odds events from [pinnacle, circa, bookmaker]
3. Parse event payload → LineSnapshot
4. Resolve fixture_id to canonical ID
5. Compute previous_price/previous_line from last known state (in-memory cache)
6. Write to TimescaleDB
7. If price != previous_price OR line != previous_line:
   → Emit to Move Detector
```

**Unabated WebSocket Worker:**
```
Same flow, different parsing logic for Unabated's message format.
Subscribe to channels for Pinnacle, Circa, Bookmaker only.
```

**The Odds API Poller:**
```
1. Poll every 60 seconds (free tier) or 10 seconds (paid tier)
2. For each fixture, compare current odds to last stored snapshot
3. If changed, create LineSnapshot and store
4. Not fast enough for real-time detection — used for gap-filling and backtesting
```

### Heartbeat & Health Monitoring

- Each worker emits a heartbeat every 10 seconds
- If no data received from a source in 60 seconds during active market hours, flag as potentially stale
- If primary (OpticOdds) goes down, promote secondary (Unabated) to primary detection source
- Log all gaps for later backfill from The Odds API historical data

---

## Layer 2: Line Store

### Database: TimescaleDB (PostgreSQL extension)

Why TimescaleDB over plain Postgres:
- Hypertable partitioning by time makes queries on recent data fast
- Built-in continuous aggregates for rollup views
- Compression for older data (lines from last month don't need raw ms precision)
- Retention policies to auto-drop granular data after N months

### Core Tables

```sql
-- Raw line snapshots (hypertable, partitioned by captured_at)
CREATE TABLE line_snapshots (
    id              UUID DEFAULT gen_random_uuid(),
    source          TEXT NOT NULL,
    book            TEXT NOT NULL,
    sport           TEXT NOT NULL,
    fixture_id      TEXT NOT NULL,       -- canonical
    market_type     TEXT NOT NULL,
    market_key      TEXT NOT NULL,
    selection       TEXT NOT NULL,
    price_decimal   DOUBLE PRECISION NOT NULL,
    price_american  INTEGER,
    line_value      DOUBLE PRECISION,
    captured_at     TIMESTAMPTZ NOT NULL,
    source_ts       TIMESTAMPTZ,
    PRIMARY KEY (id, captured_at)
);
SELECT create_hypertable('line_snapshots', 'captured_at');

-- Computed diffs (materialized on insert via trigger or continuous aggregate)
CREATE TABLE line_diffs (
    id              UUID DEFAULT gen_random_uuid(),
    book            TEXT NOT NULL,
    fixture_id      TEXT NOT NULL,
    market_key      TEXT NOT NULL,
    selection       TEXT NOT NULL,
    old_price       DOUBLE PRECISION NOT NULL,
    new_price       DOUBLE PRECISION NOT NULL,
    price_delta     DOUBLE PRECISION NOT NULL,    -- new - old in decimal
    old_line        DOUBLE PRECISION,
    new_line        DOUBLE PRECISION,
    line_delta      DOUBLE PRECISION,             -- new - old (e.g., -3 → -3.5 = -0.5)
    detected_at     TIMESTAMPTZ NOT NULL,
    time_since_last INTERVAL,                      -- gap since previous snapshot
    PRIMARY KEY (id, detected_at)
);
SELECT create_hypertable('line_diffs', 'detected_at');

-- Sharp action events (output of analysis engine)
CREATE TABLE sharp_events (
    id              UUID DEFAULT gen_random_uuid(),
    event_type      TEXT NOT NULL,         -- 'steam', 'rlm', 'origination', 'magnitude'
    fixture_id      TEXT NOT NULL,
    market_key      TEXT NOT NULL,
    selection       TEXT NOT NULL,
    direction       TEXT NOT NULL,         -- 'toward_home', 'toward_away', etc.
    confidence      DOUBLE PRECISION,      -- 0.0 to 1.0
    originating_book TEXT,                 -- which book moved first
    details         JSONB,                 -- event-type-specific metadata
    detected_at     TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, detected_at)
);
SELECT create_hypertable('sharp_events', 'detected_at');

-- Fixture metadata
CREATE TABLE fixtures (
    fixture_id      TEXT PRIMARY KEY,
    sport           TEXT NOT NULL,
    league          TEXT,
    home_team       TEXT,
    away_team       TEXT,
    start_time      TIMESTAMPTZ,
    status          TEXT DEFAULT 'scheduled'
);

-- Cross-source fixture mapping
CREATE TABLE fixture_mappings (
    source          TEXT NOT NULL,
    source_id       TEXT NOT NULL,
    fixture_id      TEXT NOT NULL REFERENCES fixtures(fixture_id),
    PRIMARY KEY (source, source_id)
);
```

### Indexes

```sql
-- Fast lookup: "show me all line movements for this fixture in the last hour"
CREATE INDEX idx_diffs_fixture_time ON line_diffs (fixture_id, detected_at DESC);

-- Fast lookup: "show me all Pinnacle moves in the last 5 minutes"
CREATE INDEX idx_diffs_book_time ON line_diffs (book, detected_at DESC);

-- Fast lookup: "show me all sharp events for this fixture"
CREATE INDEX idx_sharp_fixture ON sharp_events (fixture_id, detected_at DESC);

-- Fast lookup: "show me all steam moves today"
CREATE INDEX idx_sharp_type_time ON sharp_events (event_type, detected_at DESC);
```

### Data Retention

- Raw `line_snapshots`: Keep 7 days at full granularity, compress to 1-minute intervals after that, drop raw after 90 days
- `line_diffs`: Keep 30 days at full granularity, compress after that
- `sharp_events`: Keep indefinitely (relatively low volume, high analytical value)
- `fixtures`: Keep indefinitely

---

## Layer 3: Analysis Engine

Five detectors running in parallel, each consuming `line_diffs` in real time and emitting `sharp_events`.

### 3.1 Move Detector

The simplest detector. Fires whenever any sharp book moves a line beyond a threshold.

```
Input:  line_diff record
Logic:
    1. Is the price_delta or line_delta above the minimum threshold?
       - Spread: >= 0.5 points
       - Total: >= 0.5 points  
       - Moneyline: >= 10 cents (e.g., -110 → -120)
    2. How long since the last move on this market+book+selection?
       - If < 2 minutes since last move in same direction: accumulating sharp action
       - If > 30 minutes of stability then sudden move: high-confidence sharp trigger
    3. Emit sharp_event with type='magnitude'
       
Confidence scoring:
    - Move size relative to typical volatility for this market type
    - Time stability before the move (longer = higher confidence)
    - Time of day (moves at 3am ET on a Tuesday = very likely sharp, not public)
```

### 3.2 Steam Detector

Identifies correlated moves across two or more sharp books within a tight window.

```
Input:  line_diff record
Logic:
    1. When book A moves on fixture+market+selection:
       - Start a correlation window (configurable, default 120 seconds)
       - Watch for matching moves on same fixture+market+selection from books B and C
    2. If 2+ books move same direction within window:
       - Classify as steam move
       - Record which book moved first, second, third
       - Record time gaps between moves
    3. Emit sharp_event with type='steam'
    
Confidence scoring:
    - 3 books = higher confidence than 2
    - Tighter time window = higher confidence
    - Larger move size = higher confidence
    - All same direction AND magnitude = highest confidence
    
Implementation detail:
    - Maintain a sliding window buffer keyed by (fixture_id, market_key, selection)
    - Each entry holds recent diffs per book
    - On each new diff, check buffer for matching moves from other books
    - Expire buffer entries after window closes
```

### 3.3 Origination Tracker

Tracks which book moves first across many events to build a per-sport/per-market-type origination profile.

```
Input:  steam events (post-steam-detection)
Logic:
    1. For each steam event, record which book moved first
    2. Maintain running tallies:
       origination_counts[sport][market_type][book] += 1
    3. Compute origination rates:
       origination_rate = book_first_count / total_steam_events
    4. When a non-steam single-book move occurs:
       - Weight its significance by that book's origination rate for that sport/market
       - Pinnacle moves first 70% of the time in NFL spreads? 
         A solo Pinnacle NFL spread move gets 0.7 confidence multiplier
       - Circa moves first 60% of the time in NBA totals?
         A solo Circa NBA total move gets 0.6 confidence multiplier
    5. Periodically emit updated origination profiles (not per-event, 
       but daily recalculation)
       
This doesn't emit sharp_events directly — it feeds confidence scores 
into the other detectors.
```

### 3.4 Reverse Line Movement (RLM) Detector

Cross-references line movements with public betting percentages.

```
Input:  line_diff + public betting data
Logic:
    1. Scrape or poll public betting percentage sources:
       - Action Network (public % and money %)
       - Pregame.com
       - VegasInsider
       - (Many of these require scraping; some have informal APIs)
    2. For each line_diff:
       - Look up current public betting % for that fixture+market
       - If line moved OPPOSITE to public money direction:
         → Reverse line movement detected
       - Severity = how far apart public % and line direction are
         (80% public on Team A but line moves to Team B = very strong RLM)
    3. Emit sharp_event with type='rlm'
    
Confidence scoring:
    - Public % skew (>70% one way but line goes other = high confidence)
    - Number of sources confirming the public % (consensus = higher confidence)
    - Combined with origination data: RLM on the originating book = very high confidence
    
Limitations:
    - Public % data is often delayed 15-30 minutes
    - Public % sources sometimes disagree with each other
    - "Money %" vs "ticket %" tell different stories (few sharp bets can move money %)
    - This detector is slower / lower frequency than the others
```

### 3.5 Opening-to-Current Drift Analyzer

Not real-time — runs periodically (hourly, or on-demand) to identify sustained sharp pressure.

```
Input:  line_snapshots for a fixture from open to now
Logic:
    1. For each active fixture, retrieve opening line (first snapshot) 
       and current line (latest snapshot)
    2. Compute total drift: current - opening
    3. Compare drift to historical norms for that sport/market_type:
       - NFL spread drifting 2+ points from open = heavy sharp action
       - NBA total drifting 3+ points = heavy sharp action
    4. Track drift trajectory: steady creep vs sudden jumps
       - Steady creep = sustained sharp pressure (syndicate working the line)
       - Sudden jump = single large sharp bet or breaking news
    5. Emit sharp_event with type='drift' for fixtures exceeding thresholds
    
Use case:
    - Not for immediate tailing (too slow)
    - For identifying which side sharps are on for longer-term positions
    - For validating other detectors: if steam says "sharps on Team A" 
      and drift confirms 2-point movement toward A, high conviction
```

---

## Layer 4: Sharp Action Event Bus

### Event Format

Every detector emits events in a common format that downstream systems consume:

```
SharpActionEvent {
    id:               UUID
    event_type:       enum(steam, rlm, magnitude, drift, origination_update)
    fixture_id:       string
    sport:            string
    market_type:      string
    market_key:       string
    selection:        string
    sharp_direction:  string          // which side the sharps are on
    confidence:       float           // 0.0 to 1.0
    originating_book: string|null     // who moved first
    current_lines: {                  // snapshot of all three books at event time
        pinnacle:     { price, line }
        circa:        { price, line }
        bookmaker:    { price, line }
    }
    metadata: {
        move_size:        float|null
        time_window:      interval|null  // for steam: how long between first/last move
        public_pct:       float|null     // for RLM
        books_confirmed:  int|null       // for steam: how many books moved
        drift_from_open:  float|null     // for drift
    }
    detected_at:      timestamp(ms)
}
```

### Event Routing

Events published to a lightweight message bus (Redis Pub/Sub or NATS — either works, NATS is better if you need persistence and replay):

```
Channels:
    sharp.events.all           → every event, for logging
    sharp.events.steam         → steam moves only
    sharp.events.rlm           → reverse line movement only
    sharp.events.high_conf     → confidence > 0.75 only
    sharp.events.{sport}       → sport-specific channels
```

### Downstream Consumers

**1. Polymarket Correlation Engine**
```
Subscribes to: sharp.events.all
Action: 
    When a sharp event fires for a fixture that has a corresponding Polymarket market:
    1. Map fixture_id → Polymarket market condition_id
    2. Pull current Polymarket order book and recent trades
    3. Look for trades on Polymarket that occurred BEFORE the sharp book moved
       → These traders potentially front-ran the sharp books = candidate sharps
    4. Look for trades that occur AFTER the sharp event within 1-5 minutes
       → These traders are reacting to sharp moves = less interesting, but still data
    5. Feed candidate sharp wallets into the fingerprinting system
```

**2. Kalshi Behavioral Clustering Feed**
```
Subscribes to: sharp.events.all
Action:
    Same logic as Polymarket but without wallet attribution.
    Instead: flag the time window around each sharp event and 
    analyze Kalshi trade flow for unusual volume spikes or 
    price movements that preceded the sharp book move.
```

**3. Cross-Market Arbitrage Scanner**
```
Subscribes to: sharp.events.steam (steam = highest conviction of direction)
Action:
    When steam fires:
    1. Check Polymarket/Kalshi pricing on the same or correlated market
    2. If prediction market hasn't adjusted yet, calculate EV of tailing
    3. Feed into unified EV calculator with latency-adjusted confidence
```

**4. Alert System**
```
Subscribes to: sharp.events.high_conf
Action:
    Push notifications, Discord webhook, or dashboard alert.
    Useful for manual review and learning what patterns precede profitable trades.
```

**5. Backtesting Logger**
```
Subscribes to: sharp.events.all  
Action:
    Log every event with full context. After game completes:
    1. Grade each sharp event: did the sharp side cover?
    2. Compute: what would CLV have been if you tailed at detection time?
    3. Break down by event_type, sport, confidence level
    4. Feed accuracy metrics back into confidence scoring calibration
```

---

## Layer 5: Confidence Calibration Loop

The confidence scores from each detector are initially based on heuristics. Over time, they should be calibrated against actual outcomes.

```
Calibration cycle (weekly):
    1. Pull all sharp_events from past week with game results
    2. For each event, mark outcome: sharp side won / lost / push
    3. Group by (event_type, sport, confidence_bucket):
       e.g., "steam, NFL, confidence 0.7-0.8" → 62% win rate
    4. Adjust confidence formula weights:
       - If steam moves at confidence 0.8 are only hitting 55%, 
         reduce the weight of whatever factors push steam to 0.8
       - If RLM at confidence 0.6 is hitting 60%, 
         increase base confidence for RLM events
    5. Deploy updated weights
    
Target: confidence score should be roughly equal to win probability.
    A confidence-0.7 event should win approximately 70% of the time.
    If it's winning 60%, the model is overconfident and needs adjustment.
```

---

## Infrastructure Requirements

### Compute
- **Ingestion workers**: Lightweight, mostly I/O bound. One per aggregator source. Can run on a single machine or as separate containers.
- **Analysis engine**: CPU-bound during steam correlation. Needs low latency to line_diffs table. Co-locate with database or use in-memory buffer.
- **Event bus**: Redis or NATS. Minimal resource requirements.
- **Database**: TimescaleDB. For a single-user system, a modest Postgres instance handles this fine. ~1M line snapshots/day at peak (busy sports day across 3 books × dozens of markets × 2 selections each).

### Estimated Data Volume
- **Line snapshots**: ~500K-2M rows/day during active sports (varies wildly by season)
- **Line diffs**: ~50K-200K rows/day (only when lines actually change)
- **Sharp events**: ~100-500/day (heavily filtered)
- **Storage**: ~5GB/month raw, ~1GB/month compressed after retention policy

### Hosting
Given your existing infrastructure (TrueNAS, self-hosted services), this entire stack runs comfortably on a dedicated VM or container:
- 4 CPU cores
- 8GB RAM (mostly for TimescaleDB buffer cache)
- 100GB SSD (with compression, good for 1+ year of data)
- Stable network connection (SSE/WebSocket streams need to stay alive)

---

## Implementation Order

### Phase 1: Capture (Week 1-2)
- [ ] Sign up for OpticOdds (minimum tier covering Pinnacle + Circa + Bookmaker)
- [ ] Build ingestion normalizer with unified LineSnapshot schema
- [ ] Deploy TimescaleDB with core tables
- [ ] Build OpticOdds SSE worker, start capturing data
- [ ] Build fixture ID resolution layer (start with team+date matching)
- [ ] Verify: data flowing, no gaps, timestamps reasonable

### Phase 2: Basic Detection (Week 3-4)
- [ ] Implement Move Detector (single-book threshold moves)
- [ ] Implement Steam Detector (cross-book correlation)
- [ ] Set up event bus (Redis Pub/Sub to start)
- [ ] Build simple alert consumer (Discord webhook or similar)
- [ ] Manual review: are the detected events real? Spot-check against known sharp moves.

### Phase 3: Advanced Detection (Week 5-6)
- [ ] Implement Origination Tracker (needs 2+ weeks of steam data to be useful)
- [ ] Implement RLM Detector (requires public % data source — evaluate scraping options)
- [ ] Implement Opening-to-Current Drift Analyzer
- [ ] Cross-reference detectors: combine confidence scores from multiple detectors firing on the same event

### Phase 4: Integration (Week 7-8)
- [ ] Build Polymarket Correlation Engine consumer
- [ ] Build Kalshi Behavioral Clustering consumer
- [ ] Connect sharp events to the unified EV calculator
- [ ] Start the backtesting logger, grade past events

### Phase 5: Calibration (Ongoing)
- [ ] After 4+ weeks of graded events, run first calibration cycle
- [ ] Adjust confidence weights
- [ ] Iterate weekly
- [ ] Build dashboard for monitoring detection accuracy by sport/type/confidence

---
The fixture ID resolution problem from this document applies here too - polymarket condition_ids don't map to kalshi tickers automatically. But it's easier than sportsbook mapping because both platforms use plain-English event descriptions that can be fuzzy-matched.

## Latency Reality Check

The latency inversion risk from section 7 still applies, but reframed:

When a sharp wallet trades on polymarket:
- trade/ detects it (polling every 10s, or websocket ~instant)
- kalshi bots detect the polymarket move (their own monitors, <1s)
- your kalshi order hits the book (API call, ~200ms + 2 req/sec rate limit)

The kalshi bots are faster. But directional conviction is still valuable for the *next* entry opportunity. You won't get the exact moment, but the kalshi pipeline can weight the signal over the next few ticks.

## What's Worth Keeping from This Document

- **Steam Detector pattern** -> apply to polymarket. when multiple tracked wallets trade the same direction within a window, that's a steam signal. trade/ doesn't do this yet
- **Origination Tracker** -> track which wallets consistently move *before* polymarket price adjusts. refines existing confidence scoring
- **Confidence Calibration Loop** -> already implemented as trader_grades.py, but could formalize the weekly calibration cycle
- **SharpActionEvent schema** -> good template for the event bridge between trade/ and kalshi
- **Phase 0 Feasibility Probe** -> still relevant if you ever want to add sportsbook data. use The Odds API to validate before committing to OpticOdds

## Recommended Priority

1. **Bridge trade/ to kalshi** - polymarket sharp signals become a new scorer in pm-garden
2. **Backtest the copy-trading strategy** - replay 3-6 months of the 13 wallets' trades to validate edge
3. **Implement real execution** on polymarket (py-clob-client integration)
4. **Maybe sportsbooks** - only after 1-3 prove the edge exists

## trade/ System Gaps to Address

- Cross-market arb detection is basic (string matching on titles, needs market relationship graph)
- No backtesting module (can paper trade forward but can't validate against history)
- Real execution is stubbed (py-clob-client not integrated)
- whale_finder.py is empty (discovery of new sharps is manual)
- No connection to kalshi engine

## TL;DR

trade/ is the polymarket-native version of what this document describes. It's simpler, cheaper, and lower-latency than the sportsbook approach. The real opportunity is connecting trade/ to the kalshi engine, not building a sportsbook aggregation pipeline. This document remains useful as a reference architecture if sportsbook signals are ever needed as a third signal source.
