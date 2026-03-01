import { useColors } from "../../hooks";
import { Panel } from "../shared/Panel";
import type { Bed, Position } from "../../types";

interface ScorerDrilldownProps {
  beds: Bed[];
  positions: Position[];
  selectedSpecimen: string | null;
  selectedMarket: string | null;
}

export function ScorerDrilldown({
  beds,
  positions,
  selectedSpecimen,
  selectedMarket,
}: ScorerDrilldownProps) {
  const colors = useColors();
  // find selected specimen
  const specimen = beds
    .flatMap((b) => b.specimens)
    .find((s) => s.name === selectedSpecimen);

  // find selected position
  const position = positions.find((p) => p.ticker === selectedMarket);

  if (!specimen && !position) {
    return (
      <box style={{ flexDirection: "column", flexGrow: 1 }}>
        <Panel title="drill-down" flexGrow={1}>
          <box style={{ flexDirection: "column", gap: 1 }}>
            <text fg={colors.textDim}>
              select a scorer from [4] engine or a position from [2] positions
            </text>
            <text fg={colors.textDim}>
              then press [enter] or [l] to drill down
            </text>
            <text />
            <text fg={colors.accent}>navigation:</text>
            <text fg={colors.textDim}>  [h] go back</text>
            <text fg={colors.textDim}>  [1-7] switch tabs</text>
          </box>
        </Panel>
      </box>
    );
  }

  // specimen drill-down
  if (specimen) {
    return (
      <box style={{ flexDirection: "column", flexGrow: 1 }}>
        <Panel title={`scorer: ${specimen.name}`} marginBottom={1}>
          <box style={{ flexDirection: "column", gap: 1 }}>
            <text>
              <span fg={colors.textDim}>group: </span>
              <span fg={colors.accent}>{specimen.bed}</span>
            </text>
            <text>
              <span fg={colors.textDim}>status: </span>
              <span
                fg={
                  specimen.status === "blooming"
                    ? colors.blooming
                    : specimen.status === "dormant"
                      ? colors.dormant
                      : colors.pruned
                }
              >
                {specimen.status}
              </span>
            </text>
            <text>
              <span fg={colors.textDim}>weight: </span>
              <span fg={colors.text}>{(specimen.weight ?? 0).toFixed(3)}</span>
            </text>
            {specimen.hitRate !== undefined && (
              <text>
                <span fg={colors.textDim}>hit rate: </span>
                <span
                  fg={
                    specimen.hitRate >= 0.6
                      ? colors.success
                      : specimen.hitRate >= 0.4
                        ? colors.warning
                        : colors.error
                  }
                >
                  {(specimen.hitRate * 100).toFixed(1)}%
                </span>
              </text>
            )}
            {specimen.avgContribution !== undefined && (
              <text>
                <span fg={colors.textDim}>avg contribution: </span>
                <span
                  fg={
                    specimen.avgContribution >= 0
                      ? colors.success
                      : colors.error
                  }
                >
                  {specimen.avgContribution >= 0 ? "+" : ""}
                  {specimen.avgContribution.toFixed(3)}
                </span>
              </text>
            )}
          </box>
        </Panel>

        <Panel title="performance (simulated)" flexGrow={1}>
          <box style={{ flexDirection: "column", gap: 1 }}>
            <text fg={colors.textDim}>recent score distribution:</text>
            <text>
              <span fg={colors.success}>high (&gt;0.7): </span>
              <span fg={colors.text}>
                {"#".repeat(Math.floor(Math.random() * 10 + 5))}
              </span>
            </text>
            <text>
              <span fg={colors.warning}>mid (0.3-0.7): </span>
              <span fg={colors.text}>
                {"#".repeat(Math.floor(Math.random() * 8 + 3))}
              </span>
            </text>
            <text>
              <span fg={colors.error}>low (&lt;0.3): </span>
              <span fg={colors.text}>
                {"#".repeat(Math.floor(Math.random() * 5 + 1))}
              </span>
            </text>
            <text />
            <text fg={colors.textDim}>
              note: real performance metrics will be available once the engine
              reports scorer-level statistics
            </text>
          </box>
        </Panel>

        <Panel title="controls" marginTop={1}>
          <box style={{ flexDirection: "row", gap: 3 }}>
            <text>
              <span fg={colors.accent}>[h]</span>
              <span fg={colors.textDim}> back</span>
            </text>
            <text>
              <span fg={colors.accent}>[t]</span>
              <span fg={colors.textDim}> toggle status</span>
            </text>
            <text>
              <span fg={colors.accent}>[+/-]</span>
              <span fg={colors.textDim}> adjust weight</span>
            </text>
          </box>
        </Panel>
      </box>
    );
  }

  // position drill-down
  if (position) {
    return (
      <box style={{ flexDirection: "column", flexGrow: 1 }}>
        <Panel title={`market: ${position.ticker}`} marginBottom={1}>
          <box style={{ flexDirection: "column", gap: 1 }}>
            <text>
              <span fg={colors.textDim}>title: </span>
              <span fg={colors.text}>{position.title}</span>
            </text>
            <text>
              <span fg={colors.textDim}>category: </span>
              <span fg={colors.accent}>{position.category}</span>
            </text>
            <text>
              <span fg={colors.textDim}>side: </span>
              <span
                fg={
                  position.side === "Yes" ? colors.success : colors.error
                }
              >
                {position.side}
              </span>
            </text>
            <text>
              <span fg={colors.textDim}>quantity: </span>
              <span fg={colors.text}>{position.quantity}</span>
            </text>
            <text>
              <span fg={colors.textDim}>entry price: </span>
              <span fg={colors.text}>${(position.entryPrice ?? 0).toFixed(3)}</span>
            </text>
            {position.currentPrice != null && (
              <text>
                <span fg={colors.textDim}>current price: </span>
                <span fg={colors.text}>
                  ${position.currentPrice.toFixed(3)}
                </span>
              </text>
            )}
            <text>
              <span fg={colors.textDim}>unrealized p&l: </span>
              <span
                fg={
                  (position.unrealizedPnl ?? 0) >= 0 ? colors.success : colors.error
                }
              >
                {(position.unrealizedPnl ?? 0) >= 0 ? "+" : ""}$
                {(position.unrealizedPnl ?? 0).toFixed(2)} ({(position.pnlPct ?? 0).toFixed(1)}
                %)
              </span>
            </text>
            <text>
              <span fg={colors.textDim}>held: </span>
              <span fg={colors.text}>{position.hoursHeld}h</span>
            </text>
          </box>
        </Panel>

        <Panel title="scoring breakdown (simulated)" flexGrow={1}>
          <box style={{ flexDirection: "column", gap: 1 }}>
            <text fg={colors.textDim}>scorer contributions at entry:</text>
            {[
              { name: "momentum", score: 0.35 + Math.random() * 0.3 },
              { name: "mean_reversion", score: 0.25 + Math.random() * 0.25 },
              { name: "volume", score: 0.15 + Math.random() * 0.2 },
              { name: "time_decay", score: 0.1 + Math.random() * 0.15 },
              { name: "category_weighted", score: 0.2 + Math.random() * 0.2 },
            ].map((s) => (
              <box key={s.name} style={{ flexDirection: "row" }}>
                <text fg={colors.accent} style={{ width: 20 }}>
                  {s.name}:
                </text>
                <text fg={colors.text}>
                  {" "}
                  {renderBar(s.score)} {s.score.toFixed(3)}
                </text>
              </box>
            ))}
            <text />
            <text fg={colors.textDim}>
              note: real scorer breakdown will be available once the engine
              reports per-position scoring data
            </text>
          </box>
        </Panel>

        <Panel title="controls" marginTop={1}>
          <box style={{ flexDirection: "row", gap: 3 }}>
            <text>
              <span fg={colors.accent}>[h]</span>
              <span fg={colors.textDim}> back</span>
            </text>
          </box>
        </Panel>
      </box>
    );
  }

  return null;
}

function renderBar(value: number): string {
  const width = 15;
  const filled = Math.round(value * width);
  return "[" + "|".repeat(filled) + "-".repeat(width - filled) + "]";
}
