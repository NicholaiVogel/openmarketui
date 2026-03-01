#!/usr/bin/env python3
"""
Fetch historical trade data from Kalshi's public API with daily distribution.

Fetches a configurable number of trades per day across a date range,
ensuring good coverage rather than clustering around recent data.

Features:
- Day-by-day iteration (oldest to newest)
- Configurable trades-per-day limit
- Resume capability (tracks per-day progress)
- Retry logic with exponential backoff

Usage:
    # fetch last 2 months with default settings
    python fetch_kalshi_data_v2.py

    # fetch specific date range
    python fetch_kalshi_data_v2.py --start-date 2025-11-22 --end-date 2026-01-22

    # test with small range
    python fetch_kalshi_data_v2.py --start-date 2026-01-20 --end-date 2026-01-21
"""

import argparse
import json
import csv
import time
import urllib.request
import urllib.error
from datetime import datetime, timedelta
from pathlib import Path

BASE_URL = "https://api.elections.kalshi.com/trade-api/v2"
STATE_FILE = "fetch_state_v2.json"


def parse_args():
    parser = argparse.ArgumentParser(
        description="Fetch Kalshi trade data with daily distribution"
    )

    two_months_ago = (datetime.now() - timedelta(days=61)).strftime("%Y-%m-%d")
    today = datetime.now().strftime("%Y-%m-%d")

    parser.add_argument(
        "--start-date",
        type=str,
        default=two_months_ago,
        help=f"Start date YYYY-MM-DD (default: {two_months_ago})"
    )
    parser.add_argument(
        "--end-date",
        type=str,
        default=today,
        help=f"End date YYYY-MM-DD (default: {today})"
    )
    parser.add_argument(
        "--trades-per-day",
        type=int,
        default=100_000,
        help="Max trades to fetch per day (default: 100,000)"
    )
    parser.add_argument(
        "--output-dir",
        type=str,
        default="/mnt/work/kalshi-data/v2",
        help="Output directory (default: /mnt/work/kalshi-data/v2)"
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
    return {
        "completed_days": [],
        "current_day": None,
        "current_day_cursor": None,
        "current_day_count": 0,
        "total_trades": 0,
    }


def save_state(output_dir: Path, state: dict):
    """Save state for resuming."""
    state_path = output_dir / STATE_FILE
    with open(state_path, "w") as f:
        json.dump(state, f, indent=2)


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


def date_to_timestamps(date_str: str) -> tuple[int, int]:
    """Convert YYYY-MM-DD to (start_ts, end_ts) for that day."""
    dt = datetime.strptime(date_str, "%Y-%m-%d")
    start_ts = int(dt.timestamp())
    end_ts = int((dt + timedelta(days=1)).timestamp()) - 1
    return start_ts, end_ts


def generate_date_range(start_date: str, end_date: str) -> list[str]:
    """Generate list of YYYY-MM-DD strings from start to end (inclusive)."""
    start = datetime.strptime(start_date, "%Y-%m-%d")
    end = datetime.strptime(end_date, "%Y-%m-%d")
    dates = []
    current = start
    while current <= end:
        dates.append(current.strftime("%Y-%m-%d"))
        current += timedelta(days=1)
    return dates


def fetch_day_trades(
    output_dir: Path,
    state: dict,
    day: str,
    trades_per_day: int,
    output_path: Path,
) -> int:
    """Fetch trades for a single day. Returns count fetched."""
    min_ts, max_ts = date_to_timestamps(day)
    cursor = state["current_day_cursor"]
    count = state["current_day_count"]
    write_header = not output_path.exists()

    while count < trades_per_day:
        url = f"{BASE_URL}/markets/trades?limit=1000&min_ts={min_ts}&max_ts={max_ts}"
        if cursor:
            url += f"&cursor={cursor}"

        try:
            data = fetch_json(url)
        except Exception as e:
            print(f"    error: {e}")
            print(f"    progress saved. run again to resume.")
            return count

        batch = data.get("trades", [])
        if not batch:
            break

        append_trades_csv(batch, output_path, write_header)
        write_header = False
        count += len(batch)
        state["total_trades"] += len(batch)

        cursor = data.get("cursor")
        state["current_day_cursor"] = cursor
        state["current_day_count"] = count
        save_state(output_dir, state)

        if count % 10000 == 0 or count >= trades_per_day:
            print(f"    {day}: {count:,} trades")

        if not cursor:
            break

        time.sleep(0.3)

    return count


def main():
    args = parse_args()
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = output_dir / "trades.csv"

    print("=" * 60)
    print("Kalshi Data Fetcher v2 (daily distribution)")
    print("=" * 60)
    print(f"Date range: {args.start_date} to {args.end_date}")
    print(f"Trades per day: {args.trades_per_day:,}")
    print(f"Output: {output_path}")
    print()

    state = load_state(output_dir)
    all_days = generate_date_range(args.start_date, args.end_date)
    completed = set(state["completed_days"])

    remaining_days = [d for d in all_days if d not in completed]
    print(f"Days: {len(all_days)} total, {len(completed)} completed, "
          f"{len(remaining_days)} remaining")
    print(f"Trades so far: {state['total_trades']:,}")
    print()

    for day in remaining_days:
        # check if we're resuming this day
        if state["current_day"] == day:
            print(f"  resuming {day} from {state['current_day_count']:,} trades...")
        else:
            state["current_day"] = day
            state["current_day_cursor"] = None
            state["current_day_count"] = 0
            save_state(output_dir, state)
            print(f"  fetching {day}...")

        count = fetch_day_trades(
            output_dir, state, day, args.trades_per_day, output_path
        )

        # mark day complete
        state["completed_days"].append(day)
        state["current_day"] = None
        state["current_day_cursor"] = None
        state["current_day_count"] = 0
        save_state(output_dir, state)

        print(f"    {day} complete: {count:,} trades")

    print()
    print("=" * 60)
    print("Done!")
    print(f"Total trades: {state['total_trades']:,}")
    print(f"Days completed: {len(state['completed_days'])}")
    print(f"Output: {output_path}")
    print("=" * 60)

    return 0


if __name__ == "__main__":
    exit(main())
