import { useState, useEffect } from "react";
import { useColors, useModeStore } from "../../hooks";
import { useKeyboard } from "@opentui/react";

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

function formatBytes(n: number): string {
  const units = ["B", "KB", "MB", "GB"];
  let i = 0;
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024;
    i++;
  }
  return `${n.toFixed(1)} ${units[i]}`;
}

function formatPhase(phase?: string | null): string {
  if (!phase) return "working";
  if (phase === "fetching_trades") return "fetching trades";
  if (phase === "fetching_markets") return "fetching markets";
  if (phase === "complete") return "complete";
  if (phase === "cancelled") return "cancelled";
  return phase;
}

function getDateRange(daysBack: number): { start: string; end: string } {
  const end = new Date();
  const start = new Date();
  start.setDate(start.getDate() - daysBack);
  return {
    start: start.toISOString().split("T")[0] ?? "",
    end: end.toISOString().split("T")[0] ?? "",
  };
}

export function DataManager() {
  const colors = useColors();
  const { closeModeMenu, menuIndex } = useModeStore();

  const [availability, setAvailability] = useState<DataAvailability | null>(
    null
  );
  const [progress, setProgress] = useState<FetchProgress | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tradesPresetIndex, setTradesPresetIndex] = useState(1); // 10K default
  const [inTradesPresetMode, setInTradesPresetMode] = useState(false);

  const toggleTradesPresetMode = () => setInTradesPresetMode(!inTradesPresetMode);
  const moveTradesPresetIndex = (delta: number) => {
    const newIndex = Math.max(
      0,
      Math.min(TRADES_PER_DAY_PRESETS.length - 1, tradesPresetIndex + delta)
    );
    setTradesPresetIndex(newIndex);
  };

  useEffect(() => {
    fetchAvailability();
    const interval = setInterval(fetchStatus, 2000);
    return () => clearInterval(interval);
  }, []);

  const fetchAvailability = async () => {
    try {
      const res = await fetch(`${API_BASE}/api/data/available`);
      if (res.ok) {
        const data = (await res.json()) as DataAvailability;
        setAvailability(data);
      }
    } catch (e) {
      setError("failed to fetch data availability");
    } finally {
      setLoading(false);
    }
  };

  const fetchStatus = async () => {
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
  };

  const startFetch = async () => {
    const preset = DATE_RANGE_PRESETS[menuIndex];
    if (!preset) return;
    const { start, end } = getDateRange(preset.getDays());
    const tradesPerDay = TRADES_PER_DAY_PRESETS[tradesPresetIndex]?.value ?? 10000;

    try {
      const res = await fetch(`${API_BASE}/api/data/fetch`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          start_date: start,
          end_date: end,
          trades_per_day: tradesPerDay,
          fetch_markets: true,
          fetch_trades: true,
        }),
      });

      if (!res.ok) {
        const data = (await res.json()) as { message?: string };
        setError(data.message || "failed to start fetch");
      }
    } catch (e) {
      setError("failed to start data fetch");
    }
  };

  const cancelFetch = async () => {
    try {
      await fetch(`${API_BASE}/api/data/cancel`, { method: "POST" });
    } catch {
      // ignore
    }
  };

  const isFetching = progress?.status === "fetching";

  useKeyboard((key: any) => {
    if (isFetching) return;

    if (key.name === "t") {
      toggleTradesPresetMode();
    } else if (inTradesPresetMode) {
      if (key.name === "j" || key.name === "down") {
        moveTradesPresetIndex(1);
      } else if (key.name === "k" || key.name === "up") {
        moveTradesPresetIndex(-1);
      } else if (key.name === "escape" || key.name === "h") {
        setInTradesPresetMode(false);
      }
    }
  });

  if (loading) {
    return (
      <box style={{ flexDirection: "column" }}>
        <text fg={colors.textDim}>loading data info...</text>
      </box>
    );
  }

  if (isFetching && progress) {
    const progressPct =
      progress.days_total > 0
        ? Math.round((progress.days_complete / progress.days_total) * 100)
        : 0;
    const barWidth = 20;
    const filled = Math.round((progressPct / 100) * barWidth);
    const progressBar = "█".repeat(filled) + "░".repeat(barWidth - filled);

    return (
      <box style={{ flexDirection: "column" }}>
        <text fg={colors.accent}>downloading data</text>

        <box style={{ marginTop: 1 }}>
          <text fg={colors.text}>
            phase: {formatPhase(progress.phase)} | step: {progress.current_day || "..."} ({progress.days_complete}/
            {progress.days_total})
          </text>
        </box>

        <box style={{ marginTop: 1 }}>
          <text fg={colors.text}>
            trades: {formatNumber(progress.trades_fetched)} | markets: {formatNumber(progress.markets_fetched || 0)}
          </text>
        </box>

        <box style={{ marginTop: 1 }}>
          <text fg={colors.accent}>
            [{progressBar}] {progressPct}%
          </text>
        </box>

        <box style={{ marginTop: 2 }}>
          <text fg={colors.warning}>[esc] cancel</text>
        </box>
      </box>
    );
  }

  return (
    <box style={{ flexDirection: "column" }}>
      <text fg={colors.accent}>historical data</text>

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
        download more data:
      </text>

      <text fg={colors.textDim} style={{ marginTop: 1 }}>
        trades per day:
      </text>

      <box style={{ flexDirection: "row", marginTop: 0.5 }}>
        {TRADES_PER_DAY_PRESETS.map((preset, idx) => {
          const isSelected = idx === tradesPresetIndex;
          return (
            <box
              key={preset.label}
              style={{
                flexDirection: "row",
                backgroundColor: inTradesPresetMode && isSelected ? colors.bgAlt : undefined,
                marginRight: 1,
              }}
            >
              <text fg={inTradesPresetMode && isSelected ? colors.accent : colors.text}>
                {inTradesPresetMode && isSelected ? "[" : " "}{preset.label}{inTradesPresetMode && isSelected ? "]" : " "}
              </text>
            </box>
          );
        })}
      </box>

      <text fg={colors.textDim} style={{ marginTop: 1 }}>
        ---
      </text>

      <text fg={colors.text} style={{ marginTop: 1 }}>
        date range:
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

      <box style={{ flexDirection: "row", gap: 2, marginTop: 2 }}>
        <text fg={colors.textDim}>[enter] start download</text>
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
