import { useColors, useModeStore } from "../../hooks";
import { Panel } from "../shared/Panel";
import { SessionControlStrip } from "./SessionControlStrip";
import type {
  Bed,
  PortfolioSnapshot,
  EngineStatus,
  Fill,
  EquityPoint,
} from "../../types";

interface GardenOverviewProps {
  beds: Bed[];
  portfolio: PortfolioSnapshot | null;
  engineStatus?: EngineStatus | null;
  recentFills: Fill[];
  equityCurve: EquityPoint[];
}

function weightBar(weight: number, width: number): string {
  const maxWeight = 0.5;
  const filled = Math.min(width, Math.round((weight / maxWeight) * width));
  const empty = width - filled;
  return "\u2588".repeat(filled) + "\u2591".repeat(empty);
}

function formatUsd(value: number): string {
  return `$${value.toFixed(2)}`;
}

function formatSize(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
  return value.toFixed(2);
}

function formatAgo(timestamp: string): string {
  const ts = new Date(timestamp).getTime();
  if (Number.isNaN(ts)) return "-";
  const sec = Math.max(0, Math.floor((Date.now() - ts) / 1000));
  if (sec < 60) return `${sec}s`;
  if (sec < 3600) return `${Math.floor(sec / 60)}m`;
  if (sec < 86_400) return `${Math.floor(sec / 3600)}h`;
  return `${Math.floor(sec / 86_400)}d`;
}

function sampleSeries(values: number[], width: number): number[] {
  if (values.length <= width) return values;
  if (width <= 1) return [values[values.length - 1] ?? 0];
  const sampled: number[] = [];
  for (let i = 0; i < width; i++) {
    const idx = Math.floor((i / (width - 1)) * (values.length - 1));
    sampled.push(values[idx] ?? values[values.length - 1] ?? 0);
  }
  return sampled;
}

function buildChart(values: number[], width: number, height: number): {
  rows: string[];
  min: number;
  max: number;
} {
  if (values.length === 0) {
    return { rows: [], min: 0, max: 0 };
  }
  const sampled = sampleSeries(values, width);
  const min = Math.min(...sampled);
  const max = Math.max(...sampled);
  const span = max - min || 1;
  const levels = sampled.map((v) =>
    Math.round(((v - min) / span) * (height - 1))
  );

  const rows: string[] = [];
  for (let row = height - 1; row >= 0; row--) {
    let line = "";
    for (const level of levels) {
      line += level >= row ? "\u2588" : "\u2591";
    }
    rows.push(line);
  }

  return { rows, min, max };
}

export function GardenOverview({
  beds,
  portfolio,
  engineStatus,
  recentFills,
  equityCurve,
}: GardenOverviewProps) {
  const colors = useColors();
  const { viewMode, backtestProgress } = useModeStore();

  const allSpecimens = beds.flatMap((b) => b.specimens);
  const activeCount = allSpecimens.filter((s) => s.status === "blooming").length;
  const pausedCount = allSpecimens.filter((s) => s.status === "dormant").length;
  const disabledCount = allSpecimens.filter((s) => s.status !== "blooming" && s.status !== "dormant").length;
  const totalWeight = allSpecimens.reduce((sum, s) => sum + (s.weight ?? 0), 0);

  const isBacktestRunning =
    viewMode === "backtest" && backtestProgress.status === "running";
  const liveSnapshot = isBacktestRunning ? backtestProgress.liveSnapshot : undefined;
  const liveSeries = isBacktestRunning ? backtestProgress.liveEquitySeries : undefined;

  const equity = liveSnapshot?.equity ?? portfolio?.equity ?? 0;
  const drawdown = isBacktestRunning ? 0 : (portfolio?.drawdownPct ?? 0);
  const initialCapital = liveSnapshot?.initial_capital ?? portfolio?.initialCapital ?? 10_000;
  const settledSeries = equityCurve.map((p) => p.equity);
  const equitySeries = liveSeries && liveSeries.length > 0 ? liveSeries : settledSeries;
  const chart = buildChart(equitySeries, 52, 10);
  const latestEquity = equitySeries[equitySeries.length - 1] ?? equity;
  const firstEquity = equitySeries[0] ?? initialCapital;
  const delta = latestEquity - firstEquity;
  const deltaPct = firstEquity > 0 ? (delta / firstEquity) * 100 : 0;
  const chartColor = delta >= 0 ? colors.success : colors.error;
  const tape = recentFills.slice(0, 16);

  return (
    <box style={{ flexDirection: "column", flexGrow: 1 }}>
      {/* compact stats row */}
      <box style={{ flexDirection: "row", gap: 2, marginBottom: 1, paddingLeft: 0 }}>
        <text>
          <span fg={colors.textDim}>scorers </span>
          <span fg={colors.text}>{allSpecimens.length}</span>
        </text>
        <text>
          <span fg={colors.blooming}>{"\u25CF"}</span>
          <span fg={colors.textDim}> active </span>
          <span fg={colors.text}>{activeCount}</span>
        </text>
        <text>
          <span fg={colors.dormant}>{"\u25CB"}</span>
          <span fg={colors.textDim}> paused </span>
          <span fg={colors.text}>{pausedCount}</span>
        </text>
        {disabledCount > 0 && (
          <text>
            <span fg={colors.pruned}>{"\u2715"}</span>
            <span fg={colors.textDim}> disabled </span>
            <span fg={colors.text}>{disabledCount}</span>
          </text>
        )}
        <text>
          <span fg={colors.textDim}>groups </span>
          <span fg={colors.text}>{beds.length}</span>
        </text>
        <text>
          <span fg={colors.textDim}>total weight </span>
          <span fg={colors.text}>{totalWeight.toFixed(2)}</span>
        </text>
        <text>
          <span fg={colors.textDim}>equity </span>
          <span fg={equity >= initialCapital ? colors.success : colors.error}>
            ${equity.toFixed(2)}
          </span>
        </text>
        {drawdown > 0 && (
          <text>
            <span fg={colors.textDim}>dd </span>
            <span fg={drawdown > 5 ? colors.error : colors.warning}>
              {drawdown.toFixed(1)}%
            </span>
          </text>
        )}
      </box>

      {/* two-column layout: session strip + strategies table */}
      <box style={{ flexDirection: "row", flexGrow: 1 }}>
        <SessionControlStrip engineStatus={engineStatus ?? null} />

        <box style={{ flexDirection: "column", flexGrow: 1 }}>
          <Panel title="strategies" flexGrow={0}>
            {beds.length === 0 ? (
              <text fg={colors.textDim}>no strategies configured</text>
            ) : (
              <box style={{ flexDirection: "column" }}>
                {/* table header */}
                <box style={{ flexDirection: "row", marginBottom: 1 }}>
                  <text style={{ width: 3 }} fg={colors.textDim}> </text>
                  <text style={{ width: 20 }} fg={colors.textDim}>name</text>
                  <text style={{ width: 16 }} fg={colors.textDim}>group</text>
                  <text style={{ width: 10 }} fg={colors.textDim}>status</text>
                  <text style={{ width: 8 }} fg={colors.textDim}>weight</text>
                  <text fg={colors.textDim}>distribution</text>
                </box>

                {beds.map((bed) => (
                  <box key={bed.name} style={{ flexDirection: "column" }}>
                    {bed.specimens.map((specimen) => {
                      const weight = specimen.weight ?? 0;
                      const isActive = specimen.status === "blooming";
                      const isPaused = specimen.status === "dormant";
                      const statusColor = isActive
                        ? colors.blooming
                        : isPaused
                          ? colors.dormant
                          : colors.pruned;
                      const statusIcon = isActive ? "\u25CF" : isPaused ? "\u25CB" : "\u2715";
                      const statusLabel = isActive ? "active" : isPaused ? "paused" : "disabled";

                      return (
                        <box key={specimen.name ?? "unknown"} style={{ flexDirection: "row" }}>
                          <text style={{ width: 3 }} fg={statusColor}>{statusIcon} </text>
                          <text style={{ width: 20 }} fg={colors.text}>
                            {(specimen.name ?? "unknown").slice(0, 18)}
                          </text>
                          <text style={{ width: 16 }} fg={colors.textDim}>
                            {(bed.name).slice(0, 14)}
                          </text>
                          <text style={{ width: 10 }} fg={statusColor}>
                            {statusLabel}
                          </text>
                          <text style={{ width: 8 }} fg={colors.text}>
                            {weight.toFixed(2)}
                          </text>
                          <text fg={colors.accent}>
                            {weightBar(weight, 12)}
                          </text>
                        </box>
                      );
                    })}
                  </box>
                ))}
              </box>
            )}
          </Panel>

          <Panel title="market pulse" flexGrow={1} marginTop={1}>
            <box style={{ flexDirection: "row", flexGrow: 1 }}>
              <box style={{ flexDirection: "column", flexGrow: 1, marginRight: 2 }}>
                <box style={{ flexDirection: "row", justifyContent: "space-between", marginBottom: 1 }}>
                  <text>
                    <span fg={colors.textDim}>equity </span>
                    <span fg={colors.text}>{formatUsd(latestEquity)}</span>
                  </text>
                  <text fg={chartColor}>
                    {delta >= 0 ? "+" : ""}{formatUsd(delta)} ({delta >= 0 ? "+" : ""}{deltaPct.toFixed(2)}%)
                  </text>
                </box>

                {chart.rows.length === 0 ? (
                  <text fg={colors.textDim}>chart will populate once ticks arrive</text>
                ) : (
                  <box style={{ flexDirection: "column" }}>
                    <text fg={colors.textDim}>high {formatUsd(chart.max)}</text>
                    {chart.rows.map((row, idx) => (
                      <text key={idx} fg={chartColor}>{row}</text>
                    ))}
                    <text fg={colors.textDim}>low {formatUsd(chart.min)}</text>
                  </box>
                )}
              </box>

              <box style={{ flexDirection: "column", width: 30 }}>
                <text fg={colors.accent}>trades</text>
                <box style={{ flexDirection: "row", marginTop: 1 }}>
                  <text style={{ width: 8 }} fg={colors.textDim}>price</text>
                  <text style={{ width: 10 }} fg={colors.textDim}>size</text>
                  <text fg={colors.textDim}>time</text>
                </box>

                {tape.length === 0 ? (
                  <text fg={colors.textDim} style={{ marginTop: 1 }}>no fills yet</text>
                ) : (
                  tape.map((fill, idx) => {
                    const priceColor =
                      fill.pnl != null
                        ? fill.pnl >= 0
                          ? colors.success
                          : colors.error
                        : fill.side === "Yes"
                          ? colors.success
                          : colors.error;
                    return (
                      <box key={`${fill.ticker}-${fill.timestamp}-${idx}`} style={{ flexDirection: "row" }}>
                        <text style={{ width: 8 }} fg={priceColor}>
                          {fill.price.toFixed(2)}
                        </text>
                        <text style={{ width: 10 }} fg={colors.text}>
                          {formatSize(fill.quantity)}
                        </text>
                        <text fg={colors.textDim}>
                          {formatAgo(fill.timestamp)} ago
                        </text>
                      </box>
                    );
                  })
                )}
              </box>
            </box>
          </Panel>
        </box>
      </box>
    </box>
  );
}
