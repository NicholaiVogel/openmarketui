import { useState, useEffect, useCallback } from "react";
import { useColors, useModeStore } from "../../hooks";
import { renderProgressBar } from "../../utils/format";
import {
  describeDownloadProgress,
  formatCompactNumber,
} from "../../utils/dataProgress";

const API_BASE = process.env.PM_SERVER_URL
  ?.replace("/ws", "")
  ?.replace("ws://", "http://")
  ?.replace("wss://", "https://")
  || "http://localhost:3030";

interface DataAvailability {
  has_data: boolean;
  start_date: string | null;
  end_date: string | null;
  total_trades: number;
  total_markets: number;
  days_count: number;
  has_markets: boolean;
  has_trades: boolean;
  is_complete: boolean;
}

interface FetchProgress {
  status: "idle" | "fetching" | "complete" | "failed" | "cancelled";
  phase?: string | null;
  current_day: string | null;
  days_complete: number;
  days_total: number;
  trades_fetched: number;
  markets_fetched?: number;
  markets_done?: boolean;
  error: string | null;
}

const DATE_RANGE_PRESETS = [
  { label: "Last 7 days", getDays: () => 7 },
  { label: "Last 30 days", getDays: () => 30 },
  { label: "Last 60 days", getDays: () => 60 },
  { label: "Last 90 days", getDays: () => 90 },
  { label: "Last 6 months", getDays: () => 180 },
  { label: "Last year", getDays: () => 365 },
];

const TRADES_PER_DAY_PRESETS = [
  { label: "1K", value: 1000 },
  { label: "10K", value: 10000 },
  { label: "50K", value: 50000 },
  { label: "100K", value: 100000 },
];

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(0)}k`;
  return n.toString();
}

export function DataManager() {
  const colors = useColors();
  const { menuIndex } = useModeStore();

  const [availability, setAvailability] = useState<DataAvailability | null>(
    null
  );
  const [progress, setProgress] = useState<FetchProgress | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tradesPresetIndex, setTradesPresetIndex] = useState(1); // 10K default
  const [inTradesPresetMode, setInTradesPresetMode] = useState(false);

  const fetchAvailability = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/api/data/available`);
      if (res.ok) {
        const data = (await res.json()) as DataAvailability;
        setAvailability(data);
        setError(null);
      }
    } catch (e) {
      setError("failed to fetch data availability");
    } finally {
      setLoading(false);
    }
  }, []);

  const fetchStatus = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/api/data/status`);
      if (res.ok) {
        const data = (await res.json()) as FetchProgress;
        setProgress(data);
        if (data.status === "complete" || data.status === "failed") {
          fetchAvailability();
        }
      }
    } catch {
      // ignore
    }
  }, [fetchAvailability]);

  const isFetching = progress?.status === "fetching";

  const toggleTradesPresetMode = useCallback(() => {
    setInTradesPresetMode((prev) => !prev);
  }, []);

  const exitTradesPresetMode = useCallback(() => {
    setInTradesPresetMode(false);
  }, []);

  const moveTradesPresetIndex = useCallback((delta: number) => {
    setTradesPresetIndex((prev) =>
      Math.max(0, Math.min(TRADES_PER_DAY_PRESETS.length - 1, prev + delta))
    );
  }, []);

  const startFetch = useCallback(async () => {
    const preset = DATE_RANGE_PRESETS[menuIndex];
    if (!preset) return;

    const end = new Date();
    const start = new Date();
    start.setDate(start.getDate() - preset.getDays());

    const startStr = start.toISOString().split("T")[0] ?? "";
    const endStr = end.toISOString().split("T")[0] ?? "";
    const tradesPerDay = TRADES_PER_DAY_PRESETS[tradesPresetIndex]?.value ?? 10000;

    setError(null);
    setProgress({
      status: "fetching",
      phase: "fetching_trades",
      current_day: null,
      days_complete: 0,
      days_total: preset.getDays(),
      trades_fetched: 0,
      markets_fetched: 0,
      markets_done: false,
      error: null,
    });

    try {
      const res = await fetch(`${API_BASE}/api/data/fetch`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          start_date: startStr,
          end_date: endStr,
          trades_per_day: tradesPerDay,
          fetch_markets: true,
          fetch_trades: true,
        }),
      });

      if (!res.ok) {
        const data = (await res.json()) as { message?: string };
        setError(data.message || "failed to start download");
        setProgress((prev) =>
          prev
            ? { ...prev, status: "failed", error: data.message || "failed to start download" }
            : prev
        );
      }
    } catch {
      setError("couldn't connect to server");
      setProgress((prev) =>
        prev
          ? { ...prev, status: "failed", error: "couldn't connect to server" }
          : prev
      );
    }
  }, [menuIndex, tradesPresetIndex]);

  const cancelFetch = useCallback(async () => {
    try {
      await fetch(`${API_BASE}/api/data/cancel`, { method: "POST" });
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    fetchAvailability();
    const interval = setInterval(fetchStatus, 2000);
    return () => clearInterval(interval);
  }, [fetchAvailability, fetchStatus]);

  useEffect(() => {
    const store = globalThis as Record<string, unknown>;
    store.__dataManagerActions = {
      startFetch,
      cancelFetch,
      toggleTradesPresetMode,
      exitTradesPresetMode,
      moveTradesPresetIndex,
      isFetching: () => progress?.status === "fetching",
      isTradesPresetMode: () => inTradesPresetMode,
    };

    return () => {
      delete store.__dataManagerActions;
    };
  }, [
    startFetch,
    cancelFetch,
    toggleTradesPresetMode,
    exitTradesPresetMode,
    moveTradesPresetIndex,
    progress?.status,
    inTradesPresetMode,
  ]);

  if (loading) {
    return (
      <box style={{ flexDirection: "column" }}>
        <text fg={colors.textDim}>loading data info...</text>
      </box>
    );
  }

  if (isFetching && progress) {
    const download = describeDownloadProgress(progress);
    const progressBar = renderProgressBar(download.percent, 24);

    return (
      <box style={{ flexDirection: "column" }}>
        <text fg={colors.accent}>building backtest dataset</text>

        <box style={{ marginTop: 1 }}>
          <text fg={colors.textDim}>
            goal: replay historical markets as a trading simulation
          </text>
        </box>

        <box style={{ marginTop: 1 }}>
          <text fg={colors.text}>
            stage: {download.title}
          </text>
        </box>

        <box>
          <text fg={colors.text}>
            now: {download.currentLabel} {download.currentValue}
          </text>
        </box>

        <box style={{ marginTop: 1 }}>
          <text fg={colors.accent}>
            {progressBar} {download.percent}%
          </text>
        </box>

        <box>
          <text fg={colors.textDim}>
            {download.progressLabel}
          </text>
        </box>

        <box style={{ marginTop: 1 }}>
          <text fg={colors.text}>
            stored: {formatCompactNumber(progress.trades_fetched)} trades |{" "}
            {formatCompactNumber(progress.markets_fetched || 0)} markets
          </text>
        </box>

        <box>
          <text fg={colors.textDim}>
            trades are the price path. markets provide timing and outcomes.
          </text>
        </box>

        <box style={{ marginTop: 1 }}>
          <text fg={colors.warning}>[esc] cancel download</text>
        </box>
      </box>
    );
  }

  return (
    <box style={{ flexDirection: "column" }}>
      <text fg={colors.accent}>simulation dataset</text>

      {availability?.has_data ? (
        <box style={{ marginTop: 1, flexDirection: "column" }}>
          <text fg={colors.text}>available:</text>
          <text fg={colors.textDim}>
            {availability.start_date} to {availability.end_date}
          </text>
          <text fg={colors.textDim}>
            {formatNumber(availability.total_trades)} trades (
            {availability.days_count} days)
          </text>
          {availability.total_markets > 0 && (
            <text fg={colors.textDim}>
              {formatNumber(availability.total_markets)} markets
            </text>
          )}
          {!availability.is_complete && (
            <text fg={colors.warning}>
              ⚠ incomplete data - backtest may not work
            </text>
          )}
        </box>
      ) : (
        <box style={{ marginTop: 1 }}>
          <text fg={colors.textDim}>no data downloaded yet</text>
        </box>
      )}

      <text fg={colors.textDim} style={{ marginTop: 1 }}>
        ---
      </text>

      <text fg={colors.text} style={{ marginTop: 1 }}>
        add simulation history:
      </text>

      <text fg={colors.textDim} style={{ marginTop: 1 }}>
        coverage/day:
      </text>

      {inTradesPresetMode ? (
        <box style={{ flexDirection: "column" }}>
          {TRADES_PER_DAY_PRESETS.map((preset, idx) => {
            const isSelected = idx === tradesPresetIndex;
            return (
              <text
                key={preset.label}
                fg={isSelected ? colors.accent : colors.textDim}
              >
                {isSelected ? "> " : "  "}
                {preset.label} trades/day
              </text>
            );
          })}
        </box>
      ) : (
        <text fg={colors.text}>
          {TRADES_PER_DAY_PRESETS[tradesPresetIndex]?.label ?? "10K"} trades/day
        </text>
      )}

      <text fg={colors.textDim}>
        Higher coverage is slower. Backtests get more price detail.
      </text>

      <text fg={colors.textDim} style={{ marginTop: 1 }}>
        ---
      </text>

      <text fg={colors.text} style={{ marginTop: 1 }}>
        time window:
      </text>

      {DATE_RANGE_PRESETS.map((preset, idx) => {
        const isSelected = idx === menuIndex;
        return (
          <box
            key={preset.label}
            style={{
              flexDirection: "row",
              backgroundColor: !inTradesPresetMode && isSelected ? colors.bgAlt : undefined,
              marginRight: 1,
            }}
          >
            <text fg={!inTradesPresetMode && isSelected ? colors.accent : colors.text}>
              {!inTradesPresetMode && isSelected ? "> " : "  "}
              {preset.label}
            </text>
          </box>
        );
      })}

      {error && (
        <box style={{ marginTop: 1 }}>
          <text fg={colors.error}>error: {error}</text>
        </box>
      )}

      {progress?.status === "complete" && (
        <box style={{ marginTop: 1 }}>
          <text fg={colors.success}>download complete!</text>
        </box>
      )}

      {progress?.status === "failed" && (
        <box style={{ marginTop: 1 }}>
          <text fg={colors.error}>
            download failed: {progress.error || "unknown error"}
          </text>
        </box>
      )}

      <box style={{ flexDirection: "column", marginTop: 2 }}>
        <text fg={colors.textDim}>[enter] build dataset</text>
        {inTradesPresetMode ? (
          <>
            <text fg={colors.textDim}>[j/k] select trades/day</text>
            <text fg={colors.textDim}>[esc] exit trades/day mode</text>
          </>
        ) : (
          <>
            <text fg={colors.textDim}>[j/k] select date range</text>
            <text fg={colors.textDim}>[t] change trades/day</text>
            <text fg={colors.textDim}>[h] back</text>
          </>
        )}
      </box>
    </box>
  );
}
