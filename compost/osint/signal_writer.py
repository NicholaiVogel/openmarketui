"""Writes OsintSignal objects as JSON files to the signal directory."""

import json
import logging
import os
from pathlib import Path

from .models import OsintSignal

logger = logging.getLogger(__name__)

DEFAULT_SIGNAL_DIR = "data/osint_signals"


class SignalWriter:
    """Persists OSINT signals as individual JSON files for the Rust pipeline to consume."""

    def __init__(self, signal_dir: str = DEFAULT_SIGNAL_DIR):
        self.signal_dir = Path(signal_dir)
        self.signal_dir.mkdir(parents=True, exist_ok=True)

    def write(self, signal: OsintSignal) -> Path:
        """Write a signal to disk, returning the path to the written file."""
        ts = signal.timestamp.strftime("%Y%m%dT%H%M%S")
        filename = f"{ts}_{signal.id}.json"
        filepath = self.signal_dir / filename

        filepath.write_text(
            signal.model_dump_json(indent=2),
            encoding="utf-8",
        )

        logger.info("wrote signal %s → %s", signal.id, filepath)
        return filepath

    def cleanup_old_signals(self, max_age_hours: int = 24) -> int:
        """Remove signal files older than max_age_hours. Returns count of removed files."""
        import time

        now = time.time()
        cutoff = now - (max_age_hours * 3600)
        removed = 0

        for f in self.signal_dir.glob("*.json"):
            if f.stat().st_mtime < cutoff:
                f.unlink()
                removed += 1
                logger.debug("cleaned up old signal: %s", f.name)

        if removed:
            logger.info("cleaned up %d old signal files", removed)
        return removed
