export type TradingMode = "paper" | "backtest" | "live";
export type SessionStatus = "idle" | "running" | "paused" | "error";

export interface FeeConfig {
  takerRate: number;
  makerRate: number;
  maxPerContract: number;
  assumeTaker: boolean;
  minEdgeAfterFees: number;
}

export interface TradingConfig {
  name: string;
  initialCapital: number;
  maxPositions: number;
  kellyFraction: number;
  maxPositionPct: number;
  takeProfitPct: number;
  stopLossPct: number;
  maxHoldHours: number;
  minTimeToCloseHours: number;
  maxTimeToCloseHours: number;
  cashReservePct: number;
  maxEntriesPerTick: number;
  fees: FeeConfig;
  backtestStart?: string;
  backtestEnd?: string;
  backtestIntervalHours?: number;
}

export interface DateRangePreset {
  name: string;
  getDates: () => { start: string; end: string };
}

export interface BacktestProgress {
  status: "idle" | "running" | "complete" | "failed";
  phase?: string;
  progressPct?: number;
  elapsedSecs?: number;
  error?: string;
  liveSnapshot?: BacktestLiveSnapshot;
}

export interface BacktestLiveSnapshot {
  cash: number;
  invested: number;
  equity: number;
  initial_capital: number;
  return_pct: number;
  total_pnl: number;
  open_positions: number;
  fills_this_step: number;
}

export interface BacktestEquityPoint {
  timestamp: string;
  equity: number;
  cash: number;
  positions_value: number;
}

export interface BacktestTradeRecord {
  ticker: string;
  entry_time: string;
  exit_time?: string;
  side: string;
  quantity: number;
  entry_price: number;
  exit_price?: number;
  pnl?: number;
  category: string;
}

export interface BacktestResult {
  totalReturnPct: number;
  sharpeRatio: number;
  maxDrawdownPct: number;
  winRate: number;
  totalTrades: number;
  equityCurve: BacktestEquityPoint[];
  tradeLog: BacktestTradeRecord[];
}

export interface SessionState {
  mode: TradingMode | "idle";
  sessionId: string;
  tradingActive: boolean;
  startedAt?: string;
  config?: TradingConfig;
}
