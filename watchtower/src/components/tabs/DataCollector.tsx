import { useState, useEffect, useCallback } from "react";
import { useColors } from "../../hooks";
import { Panel } from "../shared/Panel";
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
  { label: "Last 7 days", days: 7 },
  { label: "Last 30 days", days: 30 },
  { label: "Last 60 days", days: 60 },
  { label: "Last 90 days", days: 90 },
  { label: "Last 6 months", days: 180 },
  { label: "Last year", days: 365 },
];

const TRADES_PER_DAY_PRESETS = [
  { label: "1K", value: 1000 },
  { label: "10K", value: 10000 },
  { label: "50K", value: 50000 },
  { label: "100K", value: 100000 },
];

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return n.toString();
}

interface DataCollectorProps {
  selectedIndex: number;
}

export function DataCollector({ selectedIndex }: DataCollectorProps) {
  const colors = useColors();

  const [availability, setAvailability] = useState<DataAvailability | null>(null);
  const [progress, setProgress] = useState<FetchProgress | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tradesPresetIndex, setTradesPresetIndex] = useState(1); // 10K default
  const [lastResult, setLastResult] = useState<"complete" | "failed" | null>(null);

  const datePresetIndex = Math.min(selectedIndex, DATE_RANGE_PRESETS.length - 1);

  const fetchAvailability = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/api/data/available`);
      if (res.ok) {
        const data = (await res.json()) as DataAvailability;
        setAvailability(data);
        setError(null);
      }
    } catch {
      setError("can't reach server");
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
        if (data.status === "complete") {
          setLastResult("complete");
          fetchAvailability();
        } else if (data.status === "failed") {
          setLastResult("failed");
          fetchAvailability();
        }
      }
    } catch {
      // ignore — availability fetch already handles connection errors
    }
  }, [fetchAvailability]);

  useEffect(() => {
    fetchAvailability();
    const interval = setInterval(fetchStatus, 2000);
    return () => clearInterval(interval);
  }, [fetchAvailability, fetchStatus]);

  const isFetching = progress?.status === "fetching";
  const download = progress ? describeDownloadProgress(progress) : null;

  const startFetch = useCallback(async (presetIndex: number) => {
    const preset = DATE_RANGE_PRESETS[presetIndex];
    if (!preset) return;

    const end = new Date();
    const start = new Date();
    start.setDate(start.getDate() - preset.days);

    const startStr = start.toISOString().split("T")[0] ?? "";
    const endStr = end.toISOString().split("T")[0] ?? "";
    const tradesPerDay = TRADES_PER_DAY_PRESETS[tradesPresetIndex]?.value ?? 10000;

    setError(null);
    setLastResult(null);
    setProgress({
      status: "fetching",
      phase: "fetching_trades",
      current_day: null,
      days_complete: 0,
      days_total: preset.days,
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
      }
    } catch {
      setError("couldn't connect to server");
    }
  }, [tradesPresetIndex]);

  const cancelFetch = useCallback(async () => {
    try {
      await fetch(`${API_BASE}/api/data/cancel`, { method: "POST" });
    } catch {
      // ignore
    }
  }, []);

  // expose actions for keyboard nav
  useEffect(() => {
    const store = (globalThis as Record<string, unknown>);
    store.__dataCollectorActions = {
      startFetch: () => startFetch(datePresetIndex),
      cancelFetch,
      cycleTradesPreset: () => {
        setTradesPresetIndex((prev) =>
          (prev + 1) % TRADES_PER_DAY_PRESETS.length
        );
      },
      isFetching: () => progress?.status === "fetching",
    };
    return () => { delete store.__dataCollectorActions; };
  }, [startFetch, cancelFetch, datePresetIndex, progress?.status]);

  if (loading) {
    return (
      <box style={{ flexDirection: "column" }}>
        <text fg={colors.textDim}>connecting to server...</text>
      </box>
    );
  }

  return (
    <box style={{ flexDirection: "column", flexGrow: 1 }}>
      {/* data inventory */}
      <Panel title="simulation dataset" marginBottom={1}>
        {error && !availability?.has_data ? (
          <box style={{ flexDirection: "column", gap: 1 }}>
            <text fg={colors.textDim}>
              No connection to server. Start the server to download and manage trade data.
            </text>
            <text>
              <span fg={colors.textDim}>status: </span>
              <span fg={colors.error}>{error}</span>
            </text>
          </box>
        ) : availability?.has_data ? (
          <box style={{ flexDirection: "column", gap: 1 }}>
            <box style={{ flexDirection: "row", gap: 3 }}>
              <text>
                <span fg={colors.textDim}>range: </span>
                <span fg={colors.text}>
                  {availability.start_date} to {availability.end_date}
                </span>
              </text>
              <text>
                <span fg={colors.textDim}>days: </span>
                <span fg={colors.text}>{availability.days_count}</span>
              </text>
            </box>
            <box style={{ flexDirection: "row", gap: 3 }}>
              <text>
                <span fg={colors.textDim}>trades: </span>
                <span fg={colors.text}>{formatNumber(availability.total_trades)}</span>
              </text>
              {availability.total_markets > 0 && (
                <text>
                  <span fg={colors.textDim}>markets: </span>
                  <span fg={colors.text}>{formatNumber(availability.total_markets)}</span>
                </text>
              )}
              <text>
                <span fg={colors.textDim}>ready: </span>
                <span fg={availability.is_complete !== false ? colors.success : colors.warning}>
                  {availability.is_complete !== false ? "yes" : "incomplete"}
                </span>
              </text>
            </box>
            {availability.is_complete === false && (
              <text fg={colors.warning}>
                Some days are missing. Download more to fill gaps.
              </text>
            )}
          </box>
        ) : (
          <text fg={colors.textDim}>
            No trade data downloaded yet. Pick a time range below and start a download.
          </text>
        )}
      </Panel>

      {/* active download progress */}
      {isFetching && progress && download && (
        <Panel title="building backtest dataset" marginBottom={1}>
          <box style={{ flexDirection: "column", gap: 1 }}>
            <text fg={colors.textDim}>
              Goal: create a historical simulation corpus for replay, training, and backtests.
            </text>

            <box style={{ flexDirection: "row", gap: 2 }}>
              <text>
                <span fg={colors.textDim}>stage: </span>
                <span fg={colors.accent}>{download.title}</span>
              </text>
              <text>
                <span fg={colors.textDim}>{download.currentLabel}: </span>
                <span fg={colors.text}>{download.currentValue}</span>
              </text>
            </box>

            <text fg={colors.textDim}>{download.description}</text>

            <box style={{ flexDirection: "row", gap: 1 }}>
              <text>
                <span fg={colors.accent}>
                  {renderProgressBar(download.percent, 36)}
                </span>
                <span fg={colors.text}> {download.percent}%</span>
              </text>
              <text fg={colors.textDim}>{download.progressLabel}</text>
            </box>

            <box style={{ flexDirection: "row", gap: 3 }}>
              <text>
                <span fg={colors.textDim}>stored trades: </span>
                <span fg={colors.text}>
                  {formatCompactNumber(progress.trades_fetched)}
                </span>
              </text>
              <text>
                <span fg={colors.textDim}>market definitions: </span>
                <span fg={colors.text}>
                  {formatCompactNumber(progress.markets_fetched || 0)}
                </span>
              </text>
            </box>
          </box>
        </Panel>
      )}

      {/* completion / error messages */}
      {lastResult === "complete" && !isFetching && (
        <Panel marginBottom={1}>
          <text fg={colors.success}>
            Download complete. Data is ready for backtesting.
          </text>
        </Panel>
      )}
      {lastResult === "failed" && !isFetching && progress?.error && (
        <Panel marginBottom={1}>
          <text>
            <span fg={colors.error}>Download failed: </span>
            <span fg={colors.textDim}>{progress.error}</span>
          </text>
        </Panel>
      )}

      {/* download settings */}
      <Panel title="add simulation history" flexGrow={1}>
        <box style={{ flexDirection: "column", gap: 1 }}>
          <text fg={colors.textDim}>
            Downloads trades first, then enriches each traded ticker with market metadata.
          </text>
          <text fg={colors.textDim}>
            Trades give the replay price path. Metadata gives open/close times and outcomes.
          </text>

          {/* time range selection */}
          <text fg={colors.textDim}>time window:</text>
          <box style={{ flexDirection: "column" }}>
            {DATE_RANGE_PRESETS.map((preset, idx) => {
              const isSelected = idx === datePresetIndex;
              return (
                <box
                  key={preset.label}
                  style={{
                    flexDirection: "row",
                    backgroundColor: isSelected ? colors.bgAlt : undefined,
                  }}
                >
                  <text fg={isSelected ? colors.accent : colors.text}>
                    {isSelected ? " > " : "   "}
                    {preset.label}
                  </text>
                </box>
              );
            })}
          </box>

          {/* trades per day */}
          <box style={{ flexDirection: "row", gap: 1, marginTop: 1 }}>
            <text fg={colors.textDim}>coverage/day:</text>
            {TRADES_PER_DAY_PRESETS.map((preset, idx) => {
              const isSelected = idx === tradesPresetIndex;
              return (
                <text key={preset.label} fg={isSelected ? colors.accent : colors.textDim}>
                  {isSelected ? `[${preset.label}]` : ` ${preset.label} `}
                </text>
              );
            })}
          </box>
          <text fg={colors.textDim}>
            Higher coverage is slower, but gives backtests more complete historical replay.
          </text>

          {error && availability?.has_data && (
            <text fg={colors.warning}>{error}</text>
          )}
        </box>
      </Panel>

      {/* controls */}
      <Panel title="controls" marginTop={1}>
        <box style={{ flexDirection: "column", gap: 1 }}>
          <box style={{ flexDirection: "row", gap: 3 }}>
            {isFetching ? (
              <text>
                <span fg={colors.accent}>[esc]</span>
                <span fg={colors.textDim}> cancel download</span>
              </text>
            ) : (
              <>
                <text>
                  <span fg={colors.accent}>[j/k]</span>
                  <span fg={colors.textDim}> time range</span>
                </text>
                <text>
                  <span fg={colors.accent}>[t]</span>
                  <span fg={colors.textDim}> trades/day</span>
                </text>
                <text>
                  <span fg={colors.accent}>[enter]</span>
                  <span fg={colors.textDim}> download</span>
                </text>
              </>
            )}
          </box>
          <box style={{ flexDirection: "row", gap: 1 }}>
            <text fg={colors.textDim}>status:</text>
            <text fg={
              isFetching ? colors.accent :
              lastResult === "complete" ? colors.success :
              lastResult === "failed" ? colors.error :
              error ? colors.warning :
              colors.textDim
            }>
              {isFetching
                ? download
                  ? `${download.title} — ${download.progressLabel}`
                  : "downloading"
                : lastResult === "complete"
                ? "download complete"
                : lastResult === "failed"
                ? `failed: ${progress?.error || "unknown"}`
                : error
                ? error
                : "ready"}
            </text>
          </box>
        </box>
      </Panel>
    </box>
  );
}
