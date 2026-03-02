import { create } from "zustand";
import type {
  TradingMode,
  SessionStatus,
  TradingConfig,
  BacktestProgress,
  BacktestResult,
  BacktestEquityPoint,
  BacktestTradeRecord,
  SessionState,
} from "../types/mode";
import { useGardenStore } from "./useGardenStore";
import { loadPresets, savePresets, DEFAULT_PRESETS } from "../config/presets";

const API_BASE = process.env.PM_SERVER_URL
  ?.replace("/ws", "")
  ?.replace("ws://", "http://")
  ?.replace("wss://", "https://")
  || "http://localhost:3030";

export type ModeMenuScreen =
  | "closed"
  | "mode_select"
  | "preset_select"
  | "config_edit"
  | "date_range"
  | "data_manager";

export interface DataAvailability {
  has_data: boolean;
  start_date: string | null;
  end_date: string | null;
  total_trades: number;
  days_count: number;
}

interface ModeStore {
  viewMode: TradingMode | "idle";
  sessionStatus: SessionStatus;
  sessionError: string | null;
  activeConfig: TradingConfig | null;

  configPresets: TradingConfig[];
  selectedPresetIndex: number;

  backtestProgress: BacktestProgress;
  backtestResult: BacktestResult | null;

  dataAvailability: DataAvailability | null;
  customStartDate: string;
  customEndDate: string;

  menuScreen: ModeMenuScreen;
  menuIndex: number;
  editingConfig: TradingConfig | null;
  editFieldIndex: number;
  dateRangeIndex: number;

  init: () => Promise<void>;
  setViewMode: (mode: TradingMode | "idle") => void;
  selectPreset: (index: number) => void;
  openModeMenu: () => void;
  closeModeMenu: () => void;
  setMenuScreen: (screen: ModeMenuScreen) => void;
  moveMenuIndex: (delta: number) => void;
  setMenuIndex: (index: number) => void;

  openConfigEditor: (config?: TradingConfig) => void;
  updateEditingField: (field: string, value: number | string | boolean) => void;
  moveEditField: (delta: number) => void;
  adjustEditValue: (delta: number) => void;
  savePreset: () => Promise<void>;
  savePresetAsNew: () => Promise<void>;
  deletePreset: (index: number) => Promise<void>;

  openDateRangePicker: () => void;
  moveDateRangeIndex: (delta: number) => void;
  selectDateRange: (start: string, end: string) => void;
  fetchDataAvailability: () => Promise<void>;
  setCustomStartDate: (date: string) => void;
  setCustomEndDate: (date: string) => void;
  getDateRangePresetCount: () => number;

  startSession: () => Promise<void>;
  stopSession: () => Promise<void>;
  fetchSessionStatus: () => Promise<void>;
  transitionToMode: (target: TradingMode) => Promise<void>;

  cyclePreset: (delta: number) => void;
  adjustBacktestSpeed: (delta: number) => void;

  startBacktest: () => Promise<void>;
  pollBacktestStatus: () => Promise<void>;
  dismissBacktestResult: () => void;
  stopBacktest: () => Promise<void>;
}

const EDIT_FIELDS = [
  { key: "name", label: "preset name", type: "string" },
  { key: "initialCapital", label: "initial capital", type: "currency", step: 1000 },
  { key: "maxPositions", label: "max positions", type: "number", step: 10 },
  { key: "kellyFraction", label: "kelly fraction", type: "decimal", step: 0.05 },
  { key: "maxPositionPct", label: "max position %", type: "percent", step: 0.01 },
  { key: "takeProfitPct", label: "take profit", type: "percent", step: 0.05 },
  { key: "stopLossPct", label: "stop loss", type: "percent", step: 0.05 },
  { key: "maxHoldHours", label: "max hold hours", type: "number", step: 6 },
  { key: "minTimeToCloseHours", label: "min time to close", type: "number", step: 1 },
  { key: "maxTimeToCloseHours", label: "max time to close", type: "number", step: 24 },
  { key: "cashReservePct", label: "cash reserve %", type: "percent", step: 0.05 },
  { key: "maxEntriesPerTick", label: "max entries/tick", type: "number", step: 1 },
] as const;

export { EDIT_FIELDS };

const MAX_LIVE_EQUITY_POINTS = 256;

function appendLiveEquityPoint(series: number[], equity: number): number[] {
  const last = series[series.length - 1];
  if (last === equity) {
    return series;
  }
  const next = [...series, equity];
  if (next.length > MAX_LIVE_EQUITY_POINTS) {
    return next.slice(next.length - MAX_LIVE_EQUITY_POINTS);
  }
  return next;
}

export const useModeStore = create<ModeStore>((set, get) => ({
  viewMode: "idle",
  sessionStatus: "idle",
  sessionError: null,
  activeConfig: null,

  configPresets: DEFAULT_PRESETS,
  selectedPresetIndex: 0,

  backtestProgress: { status: "idle" },
  backtestResult: null,

  dataAvailability: null,
  customStartDate: "",
  customEndDate: "",

  menuScreen: "closed",
  menuIndex: 0,
  editingConfig: null,
  editFieldIndex: 0,
  dateRangeIndex: 0,

  init: async () => {
    const presets = await loadPresets();
    set({ configPresets: presets, activeConfig: presets[0] || null });
    await get().fetchSessionStatus();
  },

  setViewMode: (mode) => set({ viewMode: mode }),

  selectPreset: (index) => {
    const { configPresets } = get();
    if (index >= 0 && index < configPresets.length) {
      set({ selectedPresetIndex: index, activeConfig: configPresets[index] });
    }
  },

  openModeMenu: () => set({ menuScreen: "mode_select", menuIndex: 0 }),
  closeModeMenu: () => set({ menuScreen: "closed", menuIndex: 0, editingConfig: null }),

  setMenuScreen: (screen) => set({ menuScreen: screen, menuIndex: 0 }),

  moveMenuIndex: (delta) => {
    const { menuScreen, menuIndex, configPresets } = get();
    let maxIndex = 0;

    if (menuScreen === "mode_select") {
      maxIndex = 2;
    } else if (menuScreen === "preset_select") {
      maxIndex = configPresets.length;
    } else if (menuScreen === "date_range") {
      maxIndex = get().getDateRangePresetCount();
    } else if (menuScreen === "data_manager") {
      maxIndex = 4; // 5 date presets
    }

    const newIndex = Math.max(0, Math.min(maxIndex, menuIndex + delta));
    set({ menuIndex: newIndex });
  },

  setMenuIndex: (index) => set({ menuIndex: index }),

  openConfigEditor: (config) => {
    const { activeConfig, configPresets, selectedPresetIndex } = get();
    const toEdit = config || activeConfig || configPresets[selectedPresetIndex];
    if (toEdit) {
      set({
        menuScreen: "config_edit",
        editingConfig: { ...toEdit },
        editFieldIndex: 0,
      });
    }
  },

  updateEditingField: (field, value) => {
    const { editingConfig } = get();
    if (!editingConfig) return;
    set({
      editingConfig: { ...editingConfig, [field]: value },
    });
  },

  moveEditField: (delta) => {
    const { editFieldIndex, viewMode } = get();
    const maxFields = viewMode === "backtest" ? EDIT_FIELDS.length + 2 : EDIT_FIELDS.length;
    const newIndex = Math.max(0, Math.min(maxFields - 1, editFieldIndex + delta));
    set({ editFieldIndex: newIndex });
  },

  adjustEditValue: (delta) => {
    const { editingConfig, editFieldIndex } = get();
    if (!editingConfig) return;

    if (editFieldIndex >= EDIT_FIELDS.length) {
      return;
    }

    const field = EDIT_FIELDS[editFieldIndex];
    if (!field || field.type === "string") return;

    const currentValue = (editingConfig as unknown as Record<string, unknown>)[field.key] as number;
    const step = field.step || 1;
    const newValue = Math.max(0, currentValue + delta * step);

    set({
      editingConfig: { ...editingConfig, [field.key]: newValue },
    });
  },

  savePreset: async () => {
    const { editingConfig, configPresets, selectedPresetIndex } = get();
    if (!editingConfig) return;

    const newPresets = [...configPresets];
    newPresets[selectedPresetIndex] = editingConfig;

    await savePresets(newPresets);
    set({
      configPresets: newPresets,
      activeConfig: editingConfig,
      menuScreen: "mode_select",
      editingConfig: null,
    });
  },

  savePresetAsNew: async () => {
    const { editingConfig, configPresets } = get();
    if (!editingConfig) return;

    const newName = `${editingConfig.name} (copy)`;
    const newPreset = { ...editingConfig, name: newName };
    const newPresets = [...configPresets, newPreset];

    await savePresets(newPresets);
    set({
      configPresets: newPresets,
      selectedPresetIndex: newPresets.length - 1,
      activeConfig: newPreset,
      menuScreen: "mode_select",
      editingConfig: null,
    });
  },

  deletePreset: async (index) => {
    const { configPresets, selectedPresetIndex } = get();
    if (configPresets.length <= 1) return;

    const newPresets = configPresets.filter((_, i) => i !== index);
    const newIndex = Math.min(selectedPresetIndex, newPresets.length - 1);

    await savePresets(newPresets);
    set({
      configPresets: newPresets,
      selectedPresetIndex: newIndex,
      activeConfig: newPresets[newIndex],
    });
  },

  openDateRangePicker: () => {
    set({ menuScreen: "date_range", dateRangeIndex: 0 });
    get().fetchDataAvailability();
  },

  moveDateRangeIndex: (delta) => {
    const { dateRangeIndex } = get();
    const maxIndex = get().getDateRangePresetCount(); // presets + 1 for custom
    const newIndex = Math.max(0, Math.min(maxIndex, dateRangeIndex + delta));
    set({ dateRangeIndex: newIndex });
  },

  selectDateRange: (start, end) => {
    const { editingConfig } = get();
    if (!editingConfig) return;
    set({
      editingConfig: {
        ...editingConfig,
        backtestStart: start,
        backtestEnd: end,
      },
      menuScreen: "config_edit",
    });
  },

  fetchDataAvailability: async () => {
    try {
      const response = await fetch(`${API_BASE}/api/data/available`);
      if (response.ok) {
        const data = (await response.json()) as DataAvailability;
        set({
          dataAvailability: data,
          customStartDate: data.start_date || "",
          customEndDate: data.end_date || "",
        });
      }
    } catch {
      // ignore
    }
  },

  setCustomStartDate: (date) => set({ customStartDate: date }),
  setCustomEndDate: (date) => set({ customEndDate: date }),

  getDateRangePresetCount: () => {
    const { dataAvailability } = get();
    if (!dataAvailability?.has_data) return 0;
    // 1 preset (full range) + 1 custom option
    // If more than 1 day, add first/second half presets
    if (dataAvailability.days_count > 1) {
      return 3; // full + first half + second half + custom = index 0-3
    }
    return 1; // full + custom = index 0-1
  },

  cyclePreset: (delta) => {
    const { configPresets, selectedPresetIndex, sessionStatus } = get();
    if (configPresets.length === 0 || sessionStatus === "running") return;
    const newIndex = (selectedPresetIndex + delta + configPresets.length) % configPresets.length;
    set({ selectedPresetIndex: newIndex, activeConfig: configPresets[newIndex] });
  },

  adjustBacktestSpeed: (delta) => {
    const { activeConfig } = get();
    if (!activeConfig) return;
    const current = activeConfig.backtestIntervalHours ?? 1;
    const next = Math.max(1, Math.min(24, current + delta));
    set({ activeConfig: { ...activeConfig, backtestIntervalHours: next } });
  },

  startSession: async () => {
    const { viewMode, activeConfig } = get();
    if (!activeConfig || viewMode === "idle") return;

    set({ sessionError: null }); // clear previous errors

    try {
      const response = await fetch(`${API_BASE}/api/session/start`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          mode: viewMode,
          config: {
            initial_capital: activeConfig.initialCapital,
            max_positions: activeConfig.maxPositions,
            kelly_fraction: activeConfig.kellyFraction,
            max_position_pct: activeConfig.maxPositionPct,
            take_profit_pct: activeConfig.takeProfitPct,
            stop_loss_pct: activeConfig.stopLossPct,
            max_hold_hours: activeConfig.maxHoldHours,
            min_time_to_close_hours: activeConfig.minTimeToCloseHours,
            max_time_to_close_hours: activeConfig.maxTimeToCloseHours,
            cash_reserve_pct: activeConfig.cashReservePct,
            max_entries_per_tick: activeConfig.maxEntriesPerTick,
            fees: {
              taker_rate: activeConfig.fees.takerRate,
              maker_rate: activeConfig.fees.makerRate,
              max_per_contract: activeConfig.fees.maxPerContract,
              assume_taker: activeConfig.fees.assumeTaker,
              min_edge_after_fees: activeConfig.fees.minEdgeAfterFees,
            },
            backtest_start: activeConfig.backtestStart,
            backtest_end: activeConfig.backtestEnd,
            backtest_interval_hours: activeConfig.backtestIntervalHours,
          },
        }),
      });

      if (response.ok) {
        set({ sessionStatus: "running", sessionError: null });
        if (viewMode === "backtest") {
          get().pollBacktestStatus();
        }
      } else {
        let errorMsg = `HTTP ${response.status}`;
        try {
          const errorData = await response.json();
          if (errorData && typeof errorData === "object" && "error" in errorData) {
            errorMsg = String(errorData.error);
          }
        } catch {
          // response body not JSON, use HTTP status
        }
        console.error("session start failed:", response.status, errorMsg);
        set({
          sessionStatus: "error",
          sessionError: errorMsg,
          backtestProgress: { status: "failed", error: errorMsg, liveEquitySeries: [] },
        });
      }
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : "failed to connect";
      console.error("session start error:", err);
      set({
        sessionStatus: "error",
        sessionError: errorMsg,
        backtestProgress: { status: "failed", error: errorMsg, liveEquitySeries: [] },
      });
    }
  },

  stopSession: async () => {
    try {
      await fetch(`${API_BASE}/api/session/stop`, { method: "POST" });
      // session status will be updated authoritatively by next WS message
      // set optimistically to idle for immediate UI feedback
      set({ sessionStatus: "idle" });
    } catch {
      // ignore
    }
  },

  fetchSessionStatus: async () => {
    try {
      const response = await fetch(`${API_BASE}/api/session/status`);
      if (response.ok) {
        // server sends snake_case, normalize to camelCase
        const raw = await response.json() as Record<string, unknown>;
        const data: SessionState = {
          mode: raw.mode as SessionState["mode"],
          sessionId: raw.session_id as string,
          tradingActive: raw.trading_active as boolean,
          startedAt: raw.started_at as string | undefined,
          config: raw.config as SessionState["config"],
        };
        set({
          viewMode: data.mode === "idle" ? "idle" : (data.mode as TradingMode),
          sessionStatus: data.tradingActive ? "running" : "idle",
        });
      }
    } catch {
      // server not reachable
    }
  },

  transitionToMode: async (target) => {
    const { viewMode, sessionStatus, stopSession, setViewMode, startSession } = get();

    // if we're already running a different mode, stop it first
    if (sessionStatus === "running" && viewMode !== "idle" && viewMode !== target) {
      await stopSession();
      // poll until server confirms session is idle (bounded retry)
      for (let i = 0; i < 10; i++) {
        await new Promise((r) => setTimeout(r, 100));
        try {
          const res = await fetch(`${API_BASE}/api/session/status`);
          if (res.ok) {
            const data = await res.json() as Record<string, unknown>;
            if (!data.trading_active) break;
          }
        } catch {
          break; // server unreachable, proceed anyway
        }
      }
    }

    // set the new view mode
    setViewMode(target);

    // for backtest, open config editor; for paper, start immediately
    if (target === "backtest") {
      get().openConfigEditor();
    } else if (target === "paper") {
      await startSession();
    }
    // live mode is not yet implemented
  },

  startBacktest: async () => {
    const { activeConfig } = get();
    if (!activeConfig?.backtestStart || !activeConfig?.backtestEnd) return;

    // immediately show progress overlay
    set({
      viewMode: "backtest",
      backtestProgress: {
        status: "running",
        phase: "starting...",
        progressPct: 0,
        liveEquitySeries: [],
      },
      menuScreen: "closed",
    });

    await get().startSession();
  },

  pollBacktestStatus: async () => {
    interface BacktestStatusData {
      status: "idle" | "running" | "complete" | "failed";
      phase?: string;
      progress_pct?: number;
      elapsed_secs?: number;
      error?: string;
      live_snapshot?: {
        cash: number;
        invested: number;
        equity: number;
        initial_capital: number;
        return_pct: number;
        total_pnl: number;
        open_positions: number;
        fills_this_step: number;
      };
    }

    interface BacktestResultData {
      total_return_pct: number;
      sharpe_ratio: number;
      max_drawdown_pct: number;
      win_rate: number;
      total_trades: number;
      equity_curve: BacktestEquityPoint[];
      trade_log: BacktestTradeRecord[];
    }

    try {
      const response = await fetch(`${API_BASE}/api/backtest/status`);
      if (!response.ok) return;

      const data = (await response.json()) as BacktestStatusData;
      set((state) => {
        const priorSeries = state.backtestProgress.liveEquitySeries ?? [];
        const nextSeries =
          data.status === "running" && data.live_snapshot
            ? appendLiveEquityPoint(priorSeries, data.live_snapshot.equity)
            : priorSeries;

        const progress: BacktestProgress = {
          status: data.status,
          phase: data.phase,
          progressPct: data.progress_pct,
          elapsedSecs: data.elapsed_secs,
          error: data.error,
          liveSnapshot: data.live_snapshot,
          liveEquitySeries: nextSeries,
        };

        return { backtestProgress: progress };
      });

      if (data.status === "running") {
        setTimeout(() => get().pollBacktestStatus(), 1000);
      } else if (data.status === "complete") {
        const resultResponse = await fetch(`${API_BASE}/api/backtest/result`);
        if (resultResponse.ok) {
          const result = (await resultResponse.json()) as BacktestResultData;
          const backtestResult: BacktestResult = {
            totalReturnPct: result.total_return_pct,
            sharpeRatio: result.sharpe_ratio,
            maxDrawdownPct: result.max_drawdown_pct,
            winRate: result.win_rate,
            totalTrades: result.total_trades,
            equityCurve: result.equity_curve || [],
            tradeLog: result.trade_log || [],
          };

          set({
            backtestResult,
            sessionStatus: "idle",
          });

          // sync to garden store for UI display
          useGardenStore.getState().syncFromBacktest(backtestResult);
        }
      }
    } catch {
      // ignore
    }
  },

  dismissBacktestResult: () => {
    set({
      backtestProgress: { status: "idle", liveEquitySeries: [] },
      backtestResult: null,
    });
  },

  stopBacktest: async () => {
    try {
      await fetch(`${API_BASE}/api/backtest/stop`, { method: "POST" });
      set({
        backtestProgress: { status: "idle", liveEquitySeries: [] },
        backtestResult: null,
        sessionStatus: "idle",
        viewMode: "idle",
      });
    } catch {
      // ignore
    }
  },
}));
