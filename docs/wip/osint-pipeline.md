# OSINT Intelligence Pipeline

Hybrid Python + Rust pipeline for qualitative geopolitical trading on Kalshi prediction markets.

## Architecture

```
telegram channels ──→ python listener ──→ LLM analyzer ──→ signal files (JSON)
                                                                │
                                                                ▼
                                              rust pipeline reads signals
                                                                │
                                                                ▼
                                           OsintScorer scores candidates
                                                                │
                                                                ▼
                                      existing pipeline: selector → executor
```

## Python Side: `compost/osint/`

### telegram_listener.py
- Real-time Telethon client monitoring OSINT channels
- Target channels: intelslava, wartranslated, OSINTdefender, IranIntl_En
- Pushes messages into analyzer as they arrive
- Filters for relevance before passing to LLM (keyword pre-filter to save API cost)

### analyzer.py
- LLM-powered event extraction and classification
- Input: raw telegram message text
- Output: structured signal with:
  - category (geopolitics, economic, military, political, climate)
  - entities extracted (countries, leaders, organizations, assets)
  - urgency level (BREAKING, HIGH, MEDIUM, LOW)
  - relevant kalshi market categories/tickers
  - conviction score (0.0-1.0)
  - theme tags for correlation tracking (iran-conflict, russia-ukraine, etc.)

### signal_writer.py
- Writes structured JSON signal files to `data/osint_signals/`
- One file per signal, named by timestamp + uuid
- Rust side watches this directory

## Signal Format

```json
{
  "id": "uuid",
  "timestamp": "2026-02-28T20:30:00Z",
  "source_channel": "intelslava",
  "urgency": "BREAKING",
  "category": "geopolitics",
  "entities": ["iran", "united_states", "strait_of_hormuz"],
  "summary": "US carrier group ordered to Persian Gulf",
  "raw_text": "...",
  "relevant_tickers": ["IRAN-WAR-2026", "OIL-ABOVE-80"],
  "conviction": 0.85,
  "themes": ["iran-conflict", "oil-prices"]
}
```

## Rust Side: `pm-garden`

### beds/kalshi/osint.rs — OsintScorer
- Implements `Scorer` trait
- Reads signal files from `data/osint_signals/`
- Matches signals to MarketCandidates by:
  - ticker match (direct)
  - category match (geopolitics → geopolitics markets)
  - entity/keyword match against market title
- Applies urgency multipliers:
  - BREAKING: 1.4x
  - HIGH: 1.2x
  - MEDIUM: 1.0x
  - LOW: 0.85x
- Writes `osint_conviction` score into candidate.scores
- Decays signal relevance over time (signal age penalty)

### Optional: OsintSource
- New Source implementation that watches for signals pointing to specific tickers
- Can inject candidates the pipeline wouldn't otherwise fetch
- "This market just became relevant because of breaking intel"

## Category Strategy Profiles

Inspired by Jake's meta-algo v2:
- Geopolitics: high conviction sizing (up to 20% per position)
- Economic: medium conviction (10% max)
- Political: medium-low (8% max)
- Climate/weather: low conviction unless BREAKING (5% max)

## Correlation/Concentration Limits

Theme-based exposure caps:
- Max portfolio % per theme (e.g. 30% max on iran-conflict)
- Conviction-gated overrides for BREAKING signals

## Reference

Architecture inspired by Jake's Buba trading system (Python, Polymarket+Kalshi):
- telegram_listener.py + telegram_monitor.py for OSINT ingestion
- signal_linker.py for trade attribution
- meta_algo.py v2 for categorization and routing
- event_pipeline.py for reactive watchers
