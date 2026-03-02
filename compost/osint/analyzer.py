"""LLM-powered geopolitical event analysis for prediction market trading."""

import json
import logging
import os
from typing import Optional

from openai import AsyncOpenAI

from .models import AnalysisResult

logger = logging.getLogger(__name__)

SYSTEM_PROMPT = """You are a geopolitical intelligence analyst specialized in extracting tradeable signals from OSINT sources for prediction markets (primarily Kalshi).

Your job is to analyze raw messages from Telegram OSINT channels and produce structured intelligence assessments.

For each message, determine:

1. **is_relevant**: Is this message relevant to prediction markets? Filter out memes, commentary without substance, reposts of old news, and channel promotions. Be selective — only flag events that could move markets.

2. **urgency**: How time-sensitive is this?
   - BREAKING: Active, unfolding event with immediate market impact (military strikes, surprise elections, major disasters)
   - HIGH: Significant development within the last few hours that markets haven't fully priced
   - MEDIUM: Notable event that could affect markets over days/weeks
   - LOW: Background context or slow-moving trend

3. **category**: Primary classification
   - geopolitics: International relations, sanctions, diplomacy, territorial disputes
   - military: Armed conflict, weapons deployments, defense posture changes
   - economic: Trade policy, commodity shocks, central bank actions, sanctions enforcement
   - political: Elections, leadership changes, legislation, domestic policy shifts
   - climate: Extreme weather, climate policy, natural disasters

4. **entities**: Key actors, countries, organizations, and assets mentioned. Use lowercase_with_underscores (e.g., "united_states", "strait_of_hormuz", "hamas").

5. **summary**: One-sentence intelligence summary suitable for a trading desk. Be precise and factual.

6. **relevant_tickers**: Kalshi market tickers this could affect. Use patterns like:
   - IRAN-WAR-2026, RUSSIA-UKRAINE-CEASEFIRE, etc. for geopolitical events
   - OIL-ABOVE-80, INFLATION-ABOVE-3 for economic indicators
   - Use your best judgment for ticker formats — be specific

7. **conviction**: How confident are you this is a real, market-moving signal? (0.0 to 1.0)
   - 0.9+: Multiple credible sources confirming, high impact
   - 0.7-0.9: Single credible source, significant event
   - 0.5-0.7: Plausible but unconfirmed
   - Below 0.5: Rumor or low-confidence intel

8. **themes**: Tags for correlation tracking (e.g., "iran-conflict", "russia-ukraine", "oil-prices", "us-election")

Respond with valid JSON only. No markdown, no explanation outside the JSON."""

USER_PROMPT_TEMPLATE = """Analyze this OSINT message from the Telegram channel "{channel}":

---
{message}
---

Respond with a JSON object matching this schema:
{{
  "is_relevant": bool,
  "urgency": "BREAKING" | "HIGH" | "MEDIUM" | "LOW",
  "category": "geopolitics" | "economic" | "military" | "political" | "climate",
  "entities": ["entity_one", "entity_two"],
  "summary": "One-sentence intelligence summary",
  "relevant_tickers": ["TICKER-ONE", "TICKER-TWO"],
  "conviction": 0.0-1.0,
  "themes": ["theme-one", "theme-two"]
}}

If the message is not relevant to prediction markets, set is_relevant to false and provide minimal fields."""


class OsintAnalyzer:
    """Analyzes raw OSINT messages using an LLM to extract structured signals."""

    def __init__(
        self,
        model: str = "openai/gpt-4o-mini",
        api_key: Optional[str] = None,
        base_url: Optional[str] = None,
    ):
        self.model = model
        self.client = AsyncOpenAI(
            api_key=api_key or os.environ.get("OPENAI_API_KEY"),
            base_url=base_url or os.environ.get("OPENAI_BASE_URL"),
        )

    async def analyze(self, message_text: str, channel: str) -> Optional[AnalysisResult]:
        """Analyze a raw message and return a structured result, or None if irrelevant."""
        try:
            response = await self.client.chat.completions.create(
                model=self.model,
                messages=[
                    {"role": "system", "content": SYSTEM_PROMPT},
                    {
                        "role": "user",
                        "content": USER_PROMPT_TEMPLATE.format(
                            channel=channel, message=message_text
                        ),
                    },
                ],
                temperature=0.2,
                max_tokens=1024,
                response_format={"type": "json_object"},
            )

            content = response.choices[0].message.content
            if not content:
                logger.warning("empty LLM response for message from %s", channel)
                return None

            data = json.loads(content)

            if not data.get("is_relevant", False):
                logger.debug("message from %s classified as irrelevant", channel)
                return None

            return AnalysisResult(**data)

        except json.JSONDecodeError as e:
            logger.error("failed to parse LLM response as JSON: %s", e)
            return None
        except Exception as e:
            logger.error("analysis failed for message from %s: %s", channel, e)
            return None
