"""Telethon-based Telegram listener for OSINT channel monitoring."""

import asyncio
import logging
import os
import re
from typing import Callable, Coroutine, Optional

from telethon import TelegramClient, events

logger = logging.getLogger(__name__)

DEFAULT_CHANNELS = [
    "intelslava",
    "wartranslated",
    "OSINTdefender",
    "IranIntl_En",
]

# keyword pre-filter: messages must contain at least one of these to be forwarded
# to the LLM. saves API cost by filtering out irrelevant chatter.
RELEVANCE_KEYWORDS = [
    # conflict / military
    "strike", "attack", "missile", "drone", "bomb", "troops", "deploy",
    "ceasefire", "escalation", "invasion", "offensive", "retreat",
    "casualties", "killed", "airstrike", "artillery", "navy", "carrier",
    # geopolitics
    "sanctions", "embargo", "treaty", "summit", "diplomacy", "nato",
    "united nations", "security council", "annexation", "sovereignty",
    # economic triggers
    "oil", "crude", "opec", "inflation", "interest rate", "tariff",
    "default", "recession", "gdp", "trade war", "commodity",
    # political
    "election", "vote", "impeach", "resign", "coup", "protest",
    "legislation", "executive order", "parliament",
    # climate / disaster
    "hurricane", "earthquake", "tsunami", "wildfire", "flood",
    "drought", "climate", "emergency",
    # urgency markers
    "breaking", "urgent", "confirmed", "just in", "developing",
    "flash", "alert",
    # key entities
    "iran", "russia", "ukraine", "china", "taiwan", "israel",
    "palestine", "gaza", "hamas", "hezbollah", "north korea",
    "strait of hormuz", "black sea", "south china sea",
]

# compile a single regex for fast matching
_keyword_pattern = re.compile(
    "|".join(re.escape(kw) for kw in RELEVANCE_KEYWORDS),
    re.IGNORECASE,
)


def passes_keyword_filter(text: str) -> bool:
    """Check if a message contains any relevance keywords."""
    return bool(_keyword_pattern.search(text))


MessageHandler = Callable[[str, str], Coroutine[None, None, None]]


class TelegramListener:
    """Monitors Telegram OSINT channels and forwards relevant messages."""

    def __init__(
        self,
        api_id: Optional[int] = None,
        api_hash: Optional[str] = None,
        channels: Optional[list[str]] = None,
        session_name: str = "osint_listener",
    ):
        self.api_id = api_id or int(os.environ["TELEGRAM_API_ID"])
        self.api_hash = api_hash or os.environ["TELEGRAM_API_HASH"]
        self.channels = channels or DEFAULT_CHANNELS
        self.session_name = session_name
        self._handler: Optional[MessageHandler] = None
        self._client: Optional[TelegramClient] = None

    def on_message(self, handler: MessageHandler) -> None:
        """Register a callback for relevant messages. Signature: async (text, channel) -> None"""
        self._handler = handler

    async def start(self) -> None:
        """Connect to Telegram and start listening for messages."""
        if not self._handler:
            raise RuntimeError("no message handler registered — call on_message() first")

        self._client = TelegramClient(self.session_name, self.api_id, self.api_hash)
        await self._client.start()

        # resolve channel entities
        resolved = []
        for ch in self.channels:
            try:
                entity = await self._client.get_entity(ch)
                resolved.append(entity)
                logger.info("monitoring channel: %s", ch)
            except Exception as e:
                logger.warning("could not resolve channel %s: %s", ch, e)

        if not resolved:
            raise RuntimeError("no channels could be resolved — check channel names")

        handler = self._handler

        @self._client.on(events.NewMessage(chats=resolved))
        async def on_new_message(event: events.NewMessage.Event):
            text = event.message.text
            if not text:
                return

            # get channel name from the chat
            chat = await event.get_chat()
            channel_name = getattr(chat, "username", None) or getattr(chat, "title", "unknown")

            if not passes_keyword_filter(text):
                logger.debug("filtered out message from %s (no keyword match)", channel_name)
                return

            logger.info("relevant message from %s, forwarding to analyzer", channel_name)
            try:
                await handler(text, channel_name)
            except Exception as e:
                logger.error("handler error for message from %s: %s", channel_name, e)

        logger.info("listener started — monitoring %d channels", len(resolved))
        await self._client.run_until_disconnected()

    async def stop(self) -> None:
        """Disconnect from Telegram."""
        if self._client:
            await self._client.disconnect()
            logger.info("listener stopped")
