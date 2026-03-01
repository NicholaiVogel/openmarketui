#!/usr/bin/env python3
"""
Fetch historical trade and market data from Kalshi's public API.
No authentication required for public endpoints.

Features:
- Incremental saves (writes batches to disk)
- Resume capability (tracks cursor position)
- Retry logic with exponential backoff
- Date filtering for trades (--min-ts, --max-ts)

Usage:
    # fetch everything (default)
    python fetch_kalshi_data.py

    # fetch trades from last 2 months with higher limit
    python fetch_kalshi_data.py --min-ts 1763794800 --trade-limit 10000000

    # reset trades state and refetch
    python fetch_kalshi_data.py --reset-trades --min-ts 1763794800
"""

import argparse
import json
import csv
import time
import urllib.request
import urllib.error
from datetime import datetime
from pathlib import Path

BASE_URL = "https://api.elections.kalshi.com/trade-api/v2"
STATE_FILE = "fetch_state.json"


def parse_args():
    parser = argparse.ArgumentParser(description="Fetch Kalshi market and trade data")
    parser.add_argument(
        "--output-dir",
        type=str,
        default="/mnt/work/kalshi-data",
        help="Output directory for CSV files (default: /mnt/work/kalshi-data)"
    )
    parser.add_argument(
        "--trade-limit",
        type=int,
        default=1_000_000,
        help="Maximum number of trades to fetch (default: 1,000,000)"
    )
    parser.add_argument(
        "--min-ts",
        type=int,
        default=None,
        help="Minimum unix timestamp for trades (trades after this time)"
    )
    parser.add_argument(
        "--max-ts",
        type=int,
        default=None,
        help="Maximum unix timestamp for trades (trades before this time)"
    )
    parser.add_argument(
        "--reset-trades",
        action="store_true",
        help="Reset trades state to fetch fresh (keeps markets done)"
    )
    parser.add_argument(
        "--trades-only",
        action="store_true",
        help="Skip markets fetch, only fetch trades"
    )
    return parser.parse_args()


def fetch_json(url: str, max_retries: int = 5) -> dict:
    """Fetch JSON from URL with retries and exponential backoff."""
    req = urllib.request.Request(url, headers={"Accept": "application/json"})

    for attempt in range(max_retries):
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                return json.loads(resp.read().decode())
        except (urllib.error.HTTPError, urllib.error.URLError) as e:
            wait = 2 ** attempt
            print(f"  attempt {attempt + 1}/{max_retries} failed: {e}")
            if attempt < max_retries - 1:
                print(f"  retrying in {wait}s...")
                time.sleep(wait)
            else:
                raise
        except Exception as e:
            wait = 2 ** attempt
            print(f"  unexpected error: {e}")
            if attempt < max_retries - 1:
                print(f"  retrying in {wait}s...")
                time.sleep(wait)
            else:
                raise


def load_state(output_dir: Path) -> dict:
    """Load saved state for resuming."""
    state_path = output_dir / STATE_FILE
    if state_path.exists():
        with open(state_path) as f:
            return json.load(f)
    return {"markets_cursor": None, "markets_count": 0,
            "trades_cursor": None, "trades_count": 0,
            "markets_done": False, "trades_done": False}


def save_state(output_dir: Path, state: dict):
    """Save state for resuming."""
    state_path = output_dir / STATE_FILE
    with open(state_path, "w") as f:
        json.dump(state, f)


def append_markets_csv(markets: list, output_path: Path, write_header: bool):
    """Append markets to CSV."""
    mode = "w" if write_header else "a"
    with open(output_path, mode, newline="") as f:
        writer = csv.writer(f)
        if write_header:
            writer.writerow(["ticker", "title", "category", "open_time",
                           "close_time", "result", "status", "yes_bid",
                           "yes_ask", "volume", "open_interest"])

        for m in markets:
            result = ""
            if m.get("result") == "yes":
                result = "yes"
            elif m.get("result") == "no":
                result = "no"
            elif m.get("status") == "finalized" and m.get("result"):
                result = m.get("result")

            writer.writerow([
                m.get("ticker", ""),
                m.get("title", ""),
                m.get("category", ""),
                m.get("open_time", ""),
                m.get("close_time", m.get("expiration_time", "")),
                result,
                m.get("status", ""),
                m.get("yes_bid", ""),
                m.get("yes_ask", ""),
                m.get("volume", ""),
                m.get("open_interest", ""),
            ])


def append_trades_csv(trades: list, output_path: Path, write_header: bool):
    """Append trades to CSV."""
    mode = "w" if write_header else "a"
    with open(output_path, mode, newline="") as f:
        writer = csv.writer(f)
        if write_header:
            writer.writerow(["timestamp", "ticker", "price", "volume", "taker_side"])

        for t in trades:
            price = t.get("yes_price", t.get("price", 50))
            taker_side = t.get("taker_side", "")
            if not taker_side:
                taker_side = "yes" if t.get("is_taker_side_yes", True) else "no"

            writer.writerow([
                t.get("created_time", t.get("ts", "")),
                t.get("ticker", t.get("market_ticker", "")),
                price,
                t.get("count", t.get("volume", 1)),
                taker_side,
            ])


def fetch_markets_incremental(output_dir: Path, state: dict) -> int:
    """Fetch markets incrementally with state tracking."""
    output_path = output_dir / "markets.csv"
    cursor = state["markets_cursor"]
    total = state["markets_count"]
    write_header = total == 0

    print(f"Resuming from {total} markets...")

    while True:
        url = f"{BASE_URL}/markets?limit=1000"
        if cursor:
            url += f"&cursor={cursor}"

        print(f"Fetching markets... ({total:,} so far)")

        try:
            data = fetch_json(url)
        except Exception as e:
            print(f"Error fetching markets: {e}")
            print(f"Progress saved. Run again to resume from {total:,} markets.")
            return total

        batch = data.get("markets", [])
        if batch:
            append_markets_csv(batch, output_path, write_header)
            write_header = False
            total += len(batch)

        cursor = data.get("cursor")
        state["markets_cursor"] = cursor
        state["markets_count"] = total
        save_state(output_dir, state)

        if not cursor:
            state["markets_done"] = True
            save_state(output_dir, state)
            break

        time.sleep(0.3)

    return total


def fetch_trades_incremental(
    output_dir: Path,
    state: dict,
    limit: int,
    min_ts: int = None,
    max_ts: int = None
) -> int:
    """Fetch trades incrementally with state tracking."""
    output_path = output_dir / "trades.csv"
    cursor = state["trades_cursor"]
    total = state["trades_count"]
    write_header = total == 0

    if total == 0:
        print("Starting fresh trades fetch...")
    else:
        print(f"Resuming from {total:,} trades...")

    if min_ts:
        print(f"  min_ts filter: {min_ts} ({datetime.fromtimestamp(min_ts)})")
    if max_ts:
        print(f"  max_ts filter: {max_ts} ({datetime.fromtimestamp(max_ts)})")

    while total < limit:
        url = f"{BASE_URL}/markets/trades?limit=1000"
        if cursor:
            url += f"&cursor={cursor}"
        if min_ts:
            url += f"&min_ts={min_ts}"
        if max_ts:
            url += f"&max_ts={max_ts}"

        print(f"Fetching trades... ({total:,}/{limit:,})")

        try:
            data = fetch_json(url)
        except Exception as e:
            print(f"Error fetching trades: {e}")
            print(f"Progress saved. Run again to resume from {total:,} trades.")
            return total

        batch = data.get("trades", [])
        if not batch:
            break

        append_trades_csv(batch, output_path, write_header)
        write_header = False
        total += len(batch)

        cursor = data.get("cursor")
        state["trades_cursor"] = cursor
        state["trades_count"] = total
        save_state(output_dir, state)

        if not cursor:
            state["trades_done"] = True
            save_state(output_dir, state)
            break

        time.sleep(0.3)

    return total


def main():
    args = parse_args()
    output_dir = Path(args.output_dir)
    output_dir.mkdir(exist_ok=True)

    print("=" * 50)
    print("Kalshi Data Fetcher (with resume)")
    print("=" * 50)
    print(f"Output: {output_dir}")
    print(f"Trade limit: {args.trade_limit:,}")

    state = load_state(output_dir)

    # reset trades state if requested
    if args.reset_trades:
        print("\nResetting trades state...")
        state["trades_cursor"] = None
        state["trades_count"] = 0
        state["trades_done"] = False
        save_state(output_dir, state)

    # fetch markets (skip if --trades-only)
    if not args.trades_only:
        if not state["markets_done"]:
            print("\n[1/2] Fetching markets...")
            markets_count = fetch_markets_incremental(output_dir, state)
            if state["markets_done"]:
                print(f"Markets complete: {markets_count:,}")
            else:
                print(f"Markets paused at: {markets_count:,}")
                return 1
        else:
            print(f"\n[1/2] Markets already complete: {state['markets_count']:,}")
    else:
        print("\n[1/2] Skipping markets (--trades-only)")

    # fetch trades
    if not state["trades_done"]:
        print("\n[2/2] Fetching trades...")
        trades_count = fetch_trades_incremental(
            output_dir,
            state,
            limit=args.trade_limit,
            min_ts=args.min_ts,
            max_ts=args.max_ts
        )
        if state["trades_done"]:
            print(f"Trades complete: {trades_count:,}")
        else:
            print(f"Trades paused at: {trades_count:,}")
            return 1
    else:
        print(f"\n[2/2] Trades already complete: {state['trades_count']:,}")

    print("\n" + "=" * 50)
    print("Done!")
    print(f"Markets: {state['markets_count']:,}")
    print(f"Trades: {state['trades_count']:,}")
    print(f"Output: {output_dir}")
    print("=" * 50)

    # clear state for next run
    (output_dir / STATE_FILE).unlink(missing_ok=True)

    return 0


if __name__ == "__main__":
    exit(main())
