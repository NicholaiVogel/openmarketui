"""Pydantic models for OSINT signal data."""

from datetime import datetime
from enum import Enum
from typing import Optional

from pydantic import BaseModel, Field


class Urgency(str, Enum):
    BREAKING = "BREAKING"
    HIGH = "HIGH"
    MEDIUM = "MEDIUM"
    LOW = "LOW"


class Category(str, Enum):
    GEOPOLITICS = "geopolitics"
    ECONOMIC = "economic"
    MILITARY = "military"
    POLITICAL = "political"
    CLIMATE = "climate"


class OsintSignal(BaseModel):
    """Structured intelligence signal extracted from OSINT sources."""

    id: str
    timestamp: datetime
    source_channel: str
    urgency: Urgency
    category: Category
    entities: list[str] = Field(default_factory=list)
    summary: str
    raw_text: str
    relevant_tickers: list[str] = Field(default_factory=list)
    conviction: float = Field(ge=0.0, le=1.0)
    themes: list[str] = Field(default_factory=list)


class AnalysisResult(BaseModel):
    """Raw LLM analysis output before being combined with message metadata."""

    urgency: Urgency
    category: Category
    entities: list[str] = Field(default_factory=list)
    summary: str
    relevant_tickers: list[str] = Field(default_factory=list)
    conviction: float = Field(ge=0.0, le=1.0)
    themes: list[str] = Field(default_factory=list)
    is_relevant: bool = True
