import { useColors } from "../../hooks";
import { Panel } from "../shared/Panel";
import type { MarketDecision } from "../../types";

interface DecisionFeedProps {
  decisions: MarketDecision[];
  selectedIndex: number;
}

export function DecisionFeed({
  decisions,
  selectedIndex,
}: DecisionFeedProps) {
  const colors = useColors();
  // use real decisions when available, fall back to empty state
  // (synthetic data generation removed - we now have real decision tracking)
  const displayDecisions = decisions;

  return (
    <box style={{ flexDirection: "column", flexGrow: 1 }}>
      <Panel title="decision feed [enter to drill-down]" flexGrow={1}>
        {displayDecisions.length === 0 ? (
          <box style={{ flexDirection: "column", gap: 1 }}>
            <text fg={colors.textDim}>no decisions recorded yet...</text>
            <text fg={colors.textDim}>
              decisions will appear here as the engine evaluates markets
            </text>
          </box>
        ) : (
          <box style={{ flexDirection: "column", gap: 0 }}>
            <box style={{ flexDirection: "row" }}>
              <text style={{ width: 10 }} fg={colors.textDim}>
                time
              </text>
              <text style={{ width: 18 }} fg={colors.textDim}>
                ticker
              </text>
              <text style={{ width: 8 }} fg={colors.textDim}>
                action
              </text>
              <text style={{ width: 6 }} fg={colors.textDim}>
                side
              </text>
              <text style={{ width: 8 }} fg={colors.textDim}>
                score
              </text>
              <text style={{ width: 8 }} fg={colors.textDim}>
                conf
              </text>
              <text style={{ width: 10 }} fg={colors.textDim}>
                latency
              </text>
              <text fg={colors.textDim}>top scorer</text>
            </box>
            {displayDecisions.map((decision, idx) => {
              const breakdown = decision.scorerBreakdown ?? {};
              const topScorer = Object.entries(breakdown).sort(([, a], [, b]) => b - a)[0];
              const score = decision.score ?? 0;
              const confidence = decision.confidence ?? 0;
              const timestamp = decision.timestamp
                ? new Date(decision.timestamp).toLocaleTimeString().slice(0, 8)
                : "-";

              return (
                <box
                  key={decision.id ?? idx}
                  style={{
                    flexDirection: "row",
                    backgroundColor:
                      selectedIndex === idx ? colors.bgAlt : undefined,
                  }}
                >
                  <text style={{ width: 10 }} fg={colors.textDim}>
                    {timestamp}
                  </text>
                  <text style={{ width: 18 }} fg={colors.accent}>
                    {(decision.ticker ?? "unknown").slice(0, 16)}
                  </text>
                  <text
                    style={{ width: 8 }}
                    fg={
                      decision.action === "enter"
                        ? colors.success
                        : decision.action === "exit"
                          ? colors.warning
                          : colors.textDim
                    }
                  >
                    {decision.action ?? "-"}
                  </text>
                  <text
                    style={{ width: 6 }}
                    fg={
                      decision.side === "Yes"
                        ? colors.success
                        : decision.side === "No"
                          ? colors.error
                          : colors.textDim
                    }
                  >
                    {decision.side || "-"}
                  </text>
                  <text
                    style={{ width: 8 }}
                    fg={
                      score >= 0.8
                        ? colors.success
                        : score >= 0.5
                          ? colors.warning
                          : colors.error
                    }
                  >
                    {score.toFixed(2)}
                  </text>
                  <text style={{ width: 8 }} fg={colors.text}>
                    {(confidence * 100).toFixed(0)}%
                  </text>
                  <text style={{ width: 10 }} fg={colors.textDim}>
                    {(decision as any).latencyMs != null
                      ? `${(decision as any).latencyMs}ms`
                      : "-"}
                  </text>
                  <text fg={colors.textDim}>
                    {topScorer ? `${topScorer[0]}: ${topScorer[1].toFixed(2)}` : "-"}
                  </text>
                </box>
              );
            })}
          </box>
        )}
      </Panel>

      <Panel title="legend" marginTop={1}>
        <box style={{ flexDirection: "row", gap: 3 }}>
          <text>
            <span fg={colors.success}>enter</span>
            <span fg={colors.textDim}> new position</span>
          </text>
          <text>
            <span fg={colors.warning}>exit</span>
            <span fg={colors.textDim}> closed</span>
          </text>
          <text>
            <span fg={colors.textDim}>skip</span>
            <span fg={colors.textDim}> no action</span>
          </text>
          <text>
            <span fg={colors.accent}>[j/k]</span>
            <span fg={colors.textDim}> navigate</span>
          </text>
        </box>
      </Panel>
    </box>
  );
}
