export interface SessionInfo {
  mode: "idle" | "paper" | "backtest" | "live";
  sessionId: string;
  startedAt?: string;
}

export interface EngineStatus {
  state: string;
  uptimeSecs: number;
  lastTick?: string;
  ticksCompleted: number;
}

export interface PortfolioSnapshot {
  cash: number;
  equity: number;
  initialCapital: number;
  returnPct: number;
  drawdownPct: number;
  positionsCount: number;
  realizedPnl: number;
  unrealizedPnl: number;
  totalPnl: number;
}

export interface CircuitBreakerStatus {
  status: string;
  drawdownPct: number;
  dailyLossPct: number;
  openPositions: number;
  fillsLastHour: number;
}

export interface PipelineMetrics {
  candidatesFetched: number;
  candidatesFiltered: number;
  candidatesSelected: number;
  signalsGenerated: number;
  fillsExecuted: number;
  durationMs: number;
}

export interface MarketDecision {
  id: string;
  timestamp: string;
  ticker: string;
  action: "enter" | "exit" | "skip";
  side?: "Yes" | "No";
  score: number;
  confidence: number;
  scorerBreakdown: Record<string, number>;
  reason?: string;
  latencyMs?: number;
}
