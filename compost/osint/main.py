"""OSINT intelligence pipeline entry point.

Wires together: Telegram listener → LLM analyzer → signal file writer.

Usage:
    python -m compost.osint.main

Environment variables:
    TELEGRAM_API_ID     - Telegram API ID (from my.telegram.org)
    TELEGRAM_API_HASH   - Telegram API hash
    OPENAI_API_KEY      - API key for LLM calls
    OPENAI_BASE_URL     - (optional) custom base URL (e.g. OpenRouter)
    OSINT_MODEL         - (optional) model name (default: openai/gpt-4o-mini)
    OSINT_SIGNAL_DIR    - (optional) signal output directory (default: data/osint_signals)
    OSINT_CHANNELS      - (optional) comma-separated channel list
    OSINT_CLEANUP_HOURS - (optional) remove signals older than N hours (default: 24)
"""

import asyncio
import logging
import os
import uuid
from datetime import datetime, timezone

from dotenv import load_dotenv

from .analyzer import OsintAnalyzer
from .models import OsintSignal
from .signal_writer import SignalWriter
from .telegram_listener import TelegramListener

load_dotenv()

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
)
logger = logging.getLogger(__name__)


def build_pipeline():
    """Construct and wire the pipeline components."""
    model = os.environ.get("OSINT_MODEL", "openai/gpt-4o-mini")
    signal_dir = os.environ.get("OSINT_SIGNAL_DIR", "data/osint_signals")
    cleanup_hours = int(os.environ.get("OSINT_CLEANUP_HOURS", "24"))

    channels = None
    if ch_env := os.environ.get("OSINT_CHANNELS"):
        channels = [c.strip() for c in ch_env.split(",") if c.strip()]

    analyzer = OsintAnalyzer(model=model)
    writer = SignalWriter(signal_dir=signal_dir)
    listener = TelegramListener(channels=channels)

    async def handle_message(text: str, channel: str) -> None:
        """Process a single message through the analysis pipeline."""
        result = await analyzer.analyze(text, channel)
        if result is None:
            return

        signal = OsintSignal(
            id=str(uuid.uuid4()),
            timestamp=datetime.now(timezone.utc),
            source_channel=channel,
            urgency=result.urgency,
            category=result.category,
            entities=result.entities,
            summary=result.summary,
            raw_text=text,
            relevant_tickers=result.relevant_tickers,
            conviction=result.conviction,
            themes=result.themes,
        )

        writer.write(signal)
        logger.info(
            "signal: [%s] %s (conviction=%.2f, urgency=%s)",
            signal.category.value,
            signal.summary,
            signal.conviction,
            signal.urgency.value,
        )

    listener.on_message(handle_message)

    return listener, writer, cleanup_hours


async def run() -> None:
    """Main async entry point."""
    listener, writer, cleanup_hours = build_pipeline()

    # periodic cleanup task
    async def cleanup_loop():
        while True:
            await asyncio.sleep(3600)  # every hour
            try:
                writer.cleanup_old_signals(max_age_hours=cleanup_hours)
            except Exception as e:
                logger.error("cleanup failed: %s", e)

    cleanup_task = asyncio.create_task(cleanup_loop())

    try:
        await listener.start()
    finally:
        cleanup_task.cancel()
        await listener.stop()


def main() -> None:
    """Sync entry point."""
    logger.info("starting OSINT intelligence pipeline")
    asyncio.run(run())


if __name__ == "__main__":
    main()
