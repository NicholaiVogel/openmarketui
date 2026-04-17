import { create } from "zustand";
import type {
  Bed,
  Position,
  Fill,
  EquityPoint,
  EngineStatus,
  PortfolioSnapshot,
  CircuitBreakerStatus,
  Alert,
  MarketDecision,
  TabName,
  ServerMessage,
  ClientMessage,
  SessionInfo,
} from "../types";
import type { BacktestResult, TradingMode } from "../types/mode";
import { useModeStore } from "./useModeStore";

const MAX_DECISIONS = 100;
const MAX_ALERTS = 50;

function syncSessionToModeStore(session: SessionInfo) {
  const mode = session.mode === "idle"
    ? "idle"
    : (session.mode as TradingMode);
  const running = session.mode !== "idle";
  useModeStore.setState({
    viewMode: mode,
    sessionStatus: running ? "running" : "idle",
  });
}

interface GardenStore {
  // connection
  connected: boolean;
  lastUpdate: string;
  serverVersion: string;

  // engine
  engineStatus: EngineStatus | null;
  circuitBreaker: CircuitBreakerStatus | null;

  // portfolio
  portfolio: PortfolioSnapshot | null;
  positions: Position[];
  recentFills: Fill[];
  equityCurve: EquityPoint[];

  // garden
  beds: Bed[];

  // decisions (circular buffer, last 100)
  decisions: MarketDecision[];

  // alerts
  alerts: Alert[];

  // ui state
  activeTab: TabName;
  selectedSpecimen: string | null;
  selectedMarket: string | null;
  selectedIndex: number;
  showHelp: boolean;
  positionsViewMode: "list" | "card";

  // websocket ref (set externally)
  ws: WebSocket | null;

  // actions
  setConnected: (connected: boolean) => void;
  setWs: (ws: WebSocket | null) => void;
  handleMessage: (msg: ServerMessage) => void;
  sendCommand: (cmd: ClientMessage) => void;

  // ui actions
  setActiveTab: (tab: TabName) => void;
  setSelectedSpecimen: (name: string | null) => void;
  setSelectedMarket: (ticker: string | null) => void;
  setSelectedIndex: (index: number) => void;
  moveSelection: (delta: number) => void;
  toggleHelp: () => void;

  // alert actions
  addAlert: (alert: Omit<Alert, "id" | "timestamp" | "acknowledged">) => void;
  acknowledgeAlert: (id: string) => void;
  acknowledgeAllAlerts: () => void;

  // control actions
  pauseEngine: () => void;
  resumeEngine: () => void;
  toggleSpecimen: (name: string) => void;
  adjustSpecimenWeight: (name: string, delta: number) => void;
  togglePositionsViewMode: () => void;

  // backtest sync
  syncFromBacktest: (result: BacktestResult) => void;
}

export const useGardenStore = create<GardenStore>((set, get) => ({
  // initial state
  connected: false,
  lastUpdate: "",
  serverVersion: "",

  engineStatus: null,
  circuitBreaker: null,

  portfolio: null,
  positions: [],
  recentFills: [],
  equityCurve: [],

  beds: [],
  decisions: [],
  alerts: [],

  activeTab: "overview",
  selectedSpecimen: null,
  selectedMarket: null,
  selectedIndex: 0,
  showHelp: false,
  positionsViewMode: "card",

  ws: null,

  setConnected: (connected) =>
    set({
      connected,
      lastUpdate: new Date().toLocaleTimeString(),
    }),

  setWs: (ws) => set({ ws }),

  handleMessage: (msg) => {
    const now = new Date().toLocaleTimeString();

    switch (msg.type) {
      case "Welcome":
        set({
          serverVersion: msg.version,
          lastUpdate: now,
        });
        syncSessionToModeStore(msg.session);
        break;

      case "Snapshot":
        set({
          engineStatus: msg.engine,
          portfolio: msg.portfolio,
          positions: msg.positions,
          recentFills: msg.recent_fills,
          equityCurve: msg.equity_curve,
          beds: msg.beds,
          circuitBreaker: msg.circuit_breaker,
          lastUpdate: now,
        });
        syncSessionToModeStore(msg.session);
        break;

      case "TickUpdate":
        set((state) => ({
          engineStatus: msg.engine,
          portfolio: msg.portfolio,
          positions: msg.positions,
          recentFills: msg.recent_fills,
          equityCurve: msg.equity_point
            ? [...state.equityCurve.slice(-199), msg.equity_point]
            : state.equityCurve,
          lastUpdate: now,
        }));
        syncSessionToModeStore(msg.session);
        break;

      case "PositionOpened":
        set((state) => {
          const exists = state.positions.some(
            (p) => p.ticker === msg.position.ticker
          );
          return {
            positions: exists
              ? state.positions
              : [...state.positions, msg.position],
            recentFills: [msg.fill, ...state.recentFills.slice(0, 49)],
            lastUpdate: now,
          };
        });
        get().addAlert({
          severity: "info",
          title: "Position Opened",
          message: `${msg.position.side} ${msg.position.ticker} @ $${msg.fill.price.toFixed(2)}`,
          source: "position",
        });
        break;

      case "PositionClosed":
        set((state) => ({
          positions: state.positions.filter((p) => p.ticker !== msg.ticker),
          recentFills: [msg.fill, ...state.recentFills.slice(0, 49)],
          lastUpdate: now,
        }));
        get().addAlert({
          severity: msg.pnl >= 0 ? "info" : "warning",
          title: "Position Closed",
          message: `${msg.ticker}: ${msg.pnl >= 0 ? "+" : ""}$${msg.pnl.toFixed(2)} (${msg.reason})`,
          source: "position",
        });
        break;

      case "SpecimenChanged":
        set((state) => ({
          beds: state.beds.map((bed) => ({
            ...bed,
            specimens: bed.specimens.map((s) =>
              s.name === msg.name
                ? { ...s, status: msg.status as any, weight: msg.weight }
                : s
            ),
          })),
          lastUpdate: now,
        }));
        break;

      case "CircuitBreakerTripped":
        get().addAlert({
          severity: "critical",
          title: "Circuit Breaker Tripped",
          message: `${msg.rule}: ${msg.details}`,
          source: "circuit_breaker",
        });
        break;

      case "Decision":
        set((state) => ({
          decisions: [
            {
              id: String(msg.id),
              timestamp: msg.timestamp,
              ticker: msg.ticker,
              action: msg.action,
              side: msg.side,
              score: msg.score,
              confidence: msg.confidence,
              scorerBreakdown: msg.scorer_breakdown,
              reason: msg.reason,
              latencyMs: msg.latency_ms,
            },
            ...state.decisions.slice(0, MAX_DECISIONS - 1),
          ],
          lastUpdate: now,
        }));
        break;

      case "CommandAck":
        if (!msg.success) {
          get().addAlert({
            severity: "warning",
            title: "Command Failed",
            message: msg.message || `${msg.command} failed`,
            source: "system",
          });
        }
        break;
    }
  },

  sendCommand: (cmd) => {
    const { ws } = get();
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(cmd));
    }
  },

  setActiveTab: (tab) => set({ activeTab: tab, selectedIndex: 0 }),
  setSelectedSpecimen: (name) => set({ selectedSpecimen: name }),
  setSelectedMarket: (ticker) => set({ selectedMarket: ticker }),
  setSelectedIndex: (index) => set({ selectedIndex: index }),

  moveSelection: (delta) =>
    set((state) => {
      let maxIndex = 0;
      if (state.activeTab === "positions") {
        maxIndex = state.positions.length - 1;
      } else if (state.activeTab === "trades") {
        maxIndex = state.recentFills.length - 1;
      } else if (state.activeTab === "engine") {
        maxIndex =
          state.beds.reduce((acc, bed) => acc + bed.specimens.length, 0) - 1;
      } else if (state.activeTab === "decisions") {
        maxIndex = state.decisions.length - 1;
      } else if (state.activeTab === "timeline") {
        maxIndex = Math.max(state.decisions.length, state.recentFills.length) - 1;
      } else if (state.activeTab === "data") {
        maxIndex = 5; // 6 date range presets (0-5)
      }

      const newIndex = Math.max(0, Math.min(maxIndex, state.selectedIndex + delta));
      return { selectedIndex: newIndex };
    }),

  toggleHelp: () => set((state) => ({ showHelp: !state.showHelp })),

  addAlert: (alert) =>
    set((state) => ({
      alerts: [
        {
          ...alert,
          id: crypto.randomUUID(),
          timestamp: new Date().toISOString(),
          acknowledged: false,
        },
        ...state.alerts.slice(0, MAX_ALERTS - 1),
      ],
    })),

  acknowledgeAlert: (id) =>
    set((state) => ({
      alerts: state.alerts.map((a) =>
        a.id === id ? { ...a, acknowledged: true } : a
      ),
    })),

  acknowledgeAllAlerts: () =>
    set((state) => ({
      alerts: state.alerts.map((a) => ({ ...a, acknowledged: true })),
    })),

  pauseEngine: () => {
    const { ws, sendCommand } = get();
    if (ws && ws.readyState === WebSocket.OPEN) {
      sendCommand({ type: "PauseEngine" });
    } else {
      // local toggle for offline/demo mode
      set((state) => ({
        engineStatus: state.engineStatus
          ? { ...state.engineStatus, state: "Paused (manual)" }
          : null,
      }));
    }
  },
  resumeEngine: () => {
    const { ws, sendCommand } = get();
    if (ws && ws.readyState === WebSocket.OPEN) {
      sendCommand({ type: "ResumeEngine" });
    } else {
      // local toggle for offline/demo mode
      set((state) => ({
        engineStatus: state.engineStatus
          ? { ...state.engineStatus, state: "Running" }
          : null,
      }));
    }
  },

  toggleSpecimen: (name) => {
    const { beds, ws, sendCommand } = get();
    for (const bed of beds) {
      const specimen = bed.specimens.find((s) => s.name === name);
      if (specimen) {
        const newStatus =
          specimen.status === "blooming" ? "dormant" : "blooming";
        if (ws && ws.readyState === WebSocket.OPEN) {
          sendCommand({ type: "SetSpecimenStatus", name, status: newStatus });
        } else {
          // local toggle for offline/demo mode
          set((state) => ({
            beds: state.beds.map((b) => ({
              ...b,
              specimens: b.specimens.map((s) =>
                s.name === name ? { ...s, status: newStatus as "blooming" | "dormant" } : s
              ),
            })),
          }));
        }
        break;
      }
    }
  },

  adjustSpecimenWeight: (name, delta) => {
    const { beds, ws, sendCommand } = get();
    for (const bed of beds) {
      const specimen = bed.specimens.find((s) => s.name === name);
      if (specimen) {
        const newWeight = Math.max(0, Math.min(2, specimen.weight + delta));
        if (ws && ws.readyState === WebSocket.OPEN) {
          sendCommand({ type: "SetSpecimenWeight", name, weight: newWeight });
        } else {
          // local update for offline/demo mode
          set((state) => ({
            beds: state.beds.map((b) => ({
              ...b,
              specimens: b.specimens.map((s) =>
                s.name === name ? { ...s, weight: newWeight } : s
              ),
            })),
          }));
        }
        break;
      }
    }
  },

  togglePositionsViewMode: () => {
    set((state) => ({
      positionsViewMode: state.positionsViewMode === "card" ? "list" : "card",
    }));
  },

  syncFromBacktest: (result) => {
    // convert trade_log to fills (both entry and exit fills)
    const fills: Fill[] = [];
    const decisions: MarketDecision[] = [];
    let decisionId = 0;

    for (const trade of result.tradeLog) {
      // entry fill
      fills.push({
        ticker: trade.ticker,
        side: trade.side as "Yes" | "No",
        quantity: trade.quantity,
        price: trade.entry_price,
        timestamp: trade.entry_time,
      });

      // synthesize entry decision
      decisions.push({
        id: `bt-${decisionId++}`,
        timestamp: trade.entry_time,
        ticker: trade.ticker,
        action: "enter",
        side: trade.side as "Yes" | "No",
        score: 0.5, // backtest doesn't expose scores
        confidence: 0,
        scorerBreakdown: {},
        reason: `backtest entry @ ${trade.entry_price.toFixed(2)}`,
      });

      // exit fill (if closed)
      if (trade.exit_time && trade.exit_price != null) {
        fills.push({
          ticker: trade.ticker,
          side: trade.side as "Yes" | "No",
          quantity: trade.quantity,
          price: trade.exit_price,
          timestamp: trade.exit_time,
          pnl: trade.pnl,
        });

        // synthesize exit decision
        const pnlStr = trade.pnl != null
          ? (trade.pnl >= 0 ? `+$${trade.pnl.toFixed(2)}` : `-$${Math.abs(trade.pnl).toFixed(2)}`)
          : "";
        decisions.push({
          id: `bt-${decisionId++}`,
          timestamp: trade.exit_time,
          ticker: trade.ticker,
          action: "exit",
          side: trade.side as "Yes" | "No",
          score: 0,
          confidence: 0,
          scorerBreakdown: {},
          reason: `backtest exit @ ${trade.exit_price.toFixed(2)} ${pnlStr}`.trim(),
        });
      }
    }

    // sort by timestamp
    fills.sort((a, b) => a.timestamp.localeCompare(b.timestamp));
    decisions.sort((a, b) => a.timestamp.localeCompare(b.timestamp));

    // convert equity curve, calculating drawdown
    let peak = 0;
    const equityCurve: EquityPoint[] = result.equityCurve.map((e) => {
      peak = Math.max(peak, e.equity);
      const drawdownPct = peak > 0 ? ((peak - e.equity) / peak) * 100 : 0;
      return {
        timestamp: e.timestamp,
        equity: e.equity,
        cash: e.cash,
        positionsValue: e.positions_value,
        drawdownPct,
      };
    });

    // build portfolio snapshot from final state
    const initial = result.equityCurve[0]?.equity || 10000;
    const final = result.equityCurve.at(-1);
    const portfolio: PortfolioSnapshot | null = final
      ? {
          cash: final.cash,
          equity: final.equity,
          initialCapital: initial,
          positionsValue: final.positions_value,
          returnPct: ((final.equity - initial) / initial) * 100,
          totalReturnPct: ((final.equity - initial) / initial) * 100,
          drawdownPct: result.maxDrawdownPct,
          positionsCount: 0, // backtest ends with all positions closed
          positionCount: 0,
          realizedPnl: final.equity - initial,
          unrealizedPnl: 0,
          totalPnl: final.equity - initial,
        }
      : null;

    set({
      recentFills: fills.slice(-100), // last 100 fills
      equityCurve,
      portfolio,
      positions: [],
      decisions: decisions.slice(-MAX_DECISIONS), // last 100 decisions
    });
  },
}));
