import type { Bed, Position, Fill, EquityPoint } from "./garden";
import type {
  EngineStatus,
  PortfolioSnapshot,
  CircuitBreakerStatus,
  PipelineMetrics,
  SessionInfo,
} from "./engine";

// server -> client messages
export type ServerMessage =
  | WelcomeMessage
  | SnapshotMessage
  | TickUpdateMessage
  | PositionOpenedMessage
  | PositionClosedMessage
  | SpecimenChangedMessage
  | CircuitBreakerTrippedMessage
  | DecisionMessage
  | CommandAckMessage
  | PongMessage;

export interface WelcomeMessage {
  type: "Welcome";
  version: string;
  session: SessionInfo;
}

export interface SnapshotMessage {
  type: "Snapshot";
  session: SessionInfo;
  engine: EngineStatus;
  portfolio: PortfolioSnapshot;
  positions: Position[];
  recent_fills: Fill[];
  equity_curve: EquityPoint[];
  beds: Bed[];
  circuit_breaker: CircuitBreakerStatus;
}

export interface TickUpdateMessage {
  type: "TickUpdate";
  session: SessionInfo;
  timestamp: string;
  engine: EngineStatus;
  portfolio: PortfolioSnapshot;
  positions: Position[];
  recent_fills: Fill[];
  equity_point?: EquityPoint;
  pipeline: PipelineMetrics;
}

export interface PositionOpenedMessage {
  type: "PositionOpened";
  position: Position;
  fill: Fill;
}

export interface PositionClosedMessage {
  type: "PositionClosed";
  ticker: string;
  pnl: number;
  reason: string;
  fill: Fill;
}

export interface SpecimenChangedMessage {
  type: "SpecimenChanged";
  name: string;
  status: string;
  weight: number;
}

export interface CircuitBreakerTrippedMessage {
  type: "CircuitBreakerTripped";
  timestamp: string;
  rule: string;
  details: string;
}

export interface DecisionMessage {
  type: "Decision";
  id: number;
  timestamp: string;
  ticker: string;
  action: "enter" | "exit" | "skip";
  side?: "Yes" | "No";
  score: number;
  confidence: number;
  scorer_breakdown: Record<string, number>;
  reason?: string;
  fill_id?: number;
  latency_ms?: number;
}

export interface CommandAckMessage {
  type: "CommandAck";
  command: string;
  success: boolean;
  message?: string;
}

export interface PongMessage {
  type: "Pong";
}

// client -> server messages
export type ClientMessage =
  | { type: "RequestSnapshot" }
  | { type: "Ping" }
  | { type: "PauseEngine" }
  | { type: "ResumeEngine" }
  | { type: "SetSpecimenStatus"; name: string; status: string }
  | { type: "SetSpecimenWeight"; name: string; weight: number }
  | { type: "ForceRefresh" };
