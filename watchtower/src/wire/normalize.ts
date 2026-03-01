// wire/normalize.ts
// Normalizes server messages from snake_case to camelCase at the ingress boundary.
// This keeps the transformation explicit and shallow - no magical deep conversion.

import type {
  SessionInfo,
  EngineStatus,
  PortfolioSnapshot,
  CircuitBreakerStatus,
  PipelineMetrics,
} from "../types/engine";
import type { Position, Fill, EquityPoint, Bed, Specimen } from "../types/garden";
import type { ServerMessage } from "../types/ws";

// wire types (snake_case from server)
interface WireSessionInfo {
  mode: string;
  session_id: string;
  started_at?: string;
}

interface WireEngineStatus {
  state: string;
  uptime_secs: number;
  last_tick?: string;
  ticks_completed: number;
}

interface WirePortfolio {
  cash: number;
  equity: number;
  initial_capital: number;
  return_pct: number;
  drawdown_pct: number;
  positions_count: number;
  realized_pnl: number;
  unrealized_pnl: number;
  total_pnl: number;
}

interface WirePosition {
  ticker: string;
  title: string;
  category: string;
  side: string;
  quantity: number;
  entry_price: number;
  current_price?: number;
  entry_time: string;
  unrealized_pnl: number;
  pnl_pct: number;
  hours_held: number;
}

interface WireFill {
  ticker: string;
  side: string;
  quantity: number;
  price: number;
  timestamp: string;
  fee?: number;
  pnl?: number;
  exit_reason?: string;
}

interface WireEquityPoint {
  timestamp: string;
  equity: number;
  cash: number;
  positions_value: number;
  drawdown_pct: number;
}

interface WireCircuitBreaker {
  status: string;
  drawdown_pct: number;
  daily_loss_pct: number;
  open_positions: number;
  fills_last_hour: number;
}

interface WirePipelineMetrics {
  candidates_fetched: number;
  candidates_filtered: number;
  candidates_selected: number;
  signals_generated: number;
  fills_executed: number;
  duration_ms: number;
}

interface WireSpecimen {
  name: string;
  status: string;
  weight: number;
  hit_rate?: number;
  avg_contribution?: number;
}

interface WireBed {
  name: string;
  specimens: WireSpecimen[];
}

export function normalizeSessionInfo(wire: WireSessionInfo): SessionInfo {
  return {
    mode: wire.mode as SessionInfo["mode"],
    sessionId: wire.session_id,
    startedAt: wire.started_at,
  };
}

export function normalizeEngineStatus(wire: WireEngineStatus): EngineStatus {
  return {
    state: wire.state,
    uptimeSecs: wire.uptime_secs,
    lastTick: wire.last_tick,
    ticksCompleted: wire.ticks_completed,
  };
}

export function normalizePortfolio(wire: WirePortfolio): PortfolioSnapshot {
  return {
    cash: wire.cash,
    equity: wire.equity,
    initialCapital: wire.initial_capital,
    returnPct: wire.return_pct,
    drawdownPct: wire.drawdown_pct,
    positionsCount: wire.positions_count,
    realizedPnl: wire.realized_pnl,
    unrealizedPnl: wire.unrealized_pnl,
    totalPnl: wire.total_pnl,
  };
}

export function normalizePosition(wire: WirePosition): Position {
  return {
    ticker: wire.ticker,
    title: wire.title,
    category: wire.category,
    side: wire.side as "Yes" | "No",
    quantity: wire.quantity,
    entryPrice: wire.entry_price,
    currentPrice: wire.current_price,
    entryTime: wire.entry_time,
    unrealizedPnl: wire.unrealized_pnl,
    pnlPct: wire.pnl_pct,
    hoursHeld: wire.hours_held,
  };
}

export function normalizeFill(wire: WireFill): Fill {
  return {
    ticker: wire.ticker,
    side: wire.side as "Yes" | "No",
    quantity: wire.quantity,
    price: wire.price,
    timestamp: wire.timestamp,
    fee: wire.fee,
    pnl: wire.pnl,
    exitReason: wire.exit_reason,
  };
}

export function normalizeEquityPoint(wire: WireEquityPoint): EquityPoint {
  return {
    timestamp: wire.timestamp,
    equity: wire.equity,
    cash: wire.cash,
    positionsValue: wire.positions_value,
    drawdownPct: wire.drawdown_pct,
  };
}

export function normalizeCircuitBreaker(wire: WireCircuitBreaker): CircuitBreakerStatus {
  return {
    status: wire.status,
    drawdownPct: wire.drawdown_pct,
    dailyLossPct: wire.daily_loss_pct,
    openPositions: wire.open_positions,
    fillsLastHour: wire.fills_last_hour,
  };
}

export function normalizePipelineMetrics(wire: WirePipelineMetrics): PipelineMetrics {
  return {
    candidatesFetched: wire.candidates_fetched,
    candidatesFiltered: wire.candidates_filtered,
    candidatesSelected: wire.candidates_selected,
    signalsGenerated: wire.signals_generated,
    fillsExecuted: wire.fills_executed,
    durationMs: wire.duration_ms,
  };
}

export function normalizeSpecimen(wire: WireSpecimen): Specimen {
  return {
    name: wire.name,
    bed: "", // bed is set by parent
    status: wire.status as Specimen["status"],
    weight: wire.weight,
    hitRate: wire.hit_rate,
    avgContribution: wire.avg_contribution,
  };
}

export function normalizeBed(wire: WireBed): Bed {
  return {
    name: wire.name,
    specimens: asArray<WireSpecimen>(wire.specimens).map((s) => ({
      ...normalizeSpecimen(s),
      bed: wire.name,
    })),
  };
}

function asArray<T>(val: unknown): T[] {
  return Array.isArray(val) ? val : [];
}

// main entry point: normalize any server message
export function normalizeServerMessage(raw: unknown): ServerMessage {
  const msg = raw as Record<string, unknown>;
  const type = msg.type as string;

  switch (type) {
    case "Welcome":
      return {
        type: "Welcome",
        version: msg.version as string,
        session: normalizeSessionInfo(msg.session as WireSessionInfo),
      };

    case "Snapshot":
      return {
        type: "Snapshot",
        session: normalizeSessionInfo(msg.session as WireSessionInfo),
        engine: normalizeEngineStatus(msg.engine as WireEngineStatus),
        portfolio: normalizePortfolio(msg.portfolio as WirePortfolio),
        positions: asArray<WirePosition>(msg.positions).map(normalizePosition),
        recent_fills: asArray<WireFill>(msg.recent_fills).map(normalizeFill),
        equity_curve: asArray<WireEquityPoint>(msg.equity_curve).map(normalizeEquityPoint),
        beds: asArray<WireBed>(msg.beds).map(normalizeBed),
        circuit_breaker: normalizeCircuitBreaker(msg.circuit_breaker as WireCircuitBreaker),
      };

    case "TickUpdate":
      return {
        type: "TickUpdate",
        session: normalizeSessionInfo(msg.session as WireSessionInfo),
        timestamp: msg.timestamp as string,
        engine: normalizeEngineStatus(msg.engine as WireEngineStatus),
        portfolio: normalizePortfolio(msg.portfolio as WirePortfolio),
        positions: asArray<WirePosition>(msg.positions).map(normalizePosition),
        recent_fills: asArray<WireFill>(msg.recent_fills).map(normalizeFill),
        equity_point: msg.equity_point
          ? normalizeEquityPoint(msg.equity_point as WireEquityPoint)
          : undefined,
        pipeline: normalizePipelineMetrics(msg.pipeline as WirePipelineMetrics),
      };

    case "PositionOpened":
      return {
        type: "PositionOpened",
        position: normalizePosition(msg.position as WirePosition),
        fill: normalizeFill(msg.fill as WireFill),
      };

    case "PositionClosed":
      return {
        type: "PositionClosed",
        ticker: msg.ticker as string,
        pnl: msg.pnl as number,
        reason: msg.reason as string,
        fill: normalizeFill(msg.fill as WireFill),
      };

    case "SpecimenChanged":
      return {
        type: "SpecimenChanged",
        name: msg.name as string,
        status: msg.status as string,
        weight: msg.weight as number,
      };

    case "CircuitBreakerTripped":
      return {
        type: "CircuitBreakerTripped",
        timestamp: msg.timestamp as string,
        rule: msg.rule as string,
        details: msg.details as string,
      };

    case "Decision":
      return {
        type: "Decision",
        id: msg.id as number,
        timestamp: msg.timestamp as string,
        ticker: msg.ticker as string,
        action: msg.action as "enter" | "exit" | "skip",
        side: msg.side as "Yes" | "No" | undefined,
        score: msg.score as number,
        confidence: msg.confidence as number,
        scorer_breakdown: (msg.scorer_breakdown as Record<string, number>) || {},
        reason: msg.reason as string | undefined,
        fill_id: msg.fill_id as number | undefined,
        latency_ms: msg.latency_ms as number | undefined,
      };

    case "CommandAck":
      return {
        type: "CommandAck",
        command: msg.command as string,
        success: msg.success as boolean,
        message: msg.message as string | undefined,
      };

    case "Pong":
      return { type: "Pong" };

    default:
      // pass through unknown messages as-is (with type coercion)
      return msg as unknown as ServerMessage;
  }
}
