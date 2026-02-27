import { useColors } from "../../hooks";
import { Panel } from "../shared/Panel";
import { SessionControlStrip } from "./SessionControlStrip";
import type { Bed, PortfolioSnapshot, EngineStatus } from "../../types";

interface GardenOverviewProps {
  beds: Bed[];
  portfolio: PortfolioSnapshot | null;
  engineStatus?: EngineStatus | null;
}

function weightBar(weight: number, width: number): string {
  const maxWeight = 0.5;
  const filled = Math.min(width, Math.round((weight / maxWeight) * width));
  const empty = width - filled;
  return "\u2588".repeat(filled) + "\u2591".repeat(empty);
}

export function GardenOverview({ beds, portfolio, engineStatus }: GardenOverviewProps) {
  const colors = useColors();

  const allSpecimens = beds.flatMap((b) => b.specimens);
  const activeCount = allSpecimens.filter((s) => s.status === "blooming").length;
  const pausedCount = allSpecimens.filter((s) => s.status === "dormant").length;
  const disabledCount = allSpecimens.filter((s) => s.status !== "blooming" && s.status !== "dormant").length;
  const totalWeight = allSpecimens.reduce((sum, s) => sum + (s.weight ?? 0), 0);

  const equity = portfolio?.equity ?? 0;
  const drawdown = portfolio?.drawdownPct ?? 0;

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
          <span fg={equity >= (portfolio?.initialCapital ?? 0) ? colors.success : colors.error}>
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

        <Panel title="strategies" flexGrow={1}>
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
      </box>
    </box>
  );
}
