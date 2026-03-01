import { useColors } from "../../hooks";
import { Panel } from "../shared/Panel";
import type { MarketDecision, Fill } from "../../types";

interface TransactionTimelineProps {
  decisions: MarketDecision[];
  recentFills: Fill[];
  selectedIndex: number;
}

interface TimelineEvent {
  type: "decision" | "fill";
  timestamp: string;
  ticker: string;
  decision?: MarketDecision;
  fill?: Fill;
}

function formatTime(ts: string): string {
  try {
    return new Date(ts).toLocaleTimeString().slice(0, 8);
  } catch {
    return "-";
  }
}

function groupEvents(
  decisions: MarketDecision[],
  fills: Fill[]
): TimelineEvent[][] {
  const events: TimelineEvent[] = [];

  for (const d of decisions) {
    events.push({
      type: "decision",
      timestamp: d.timestamp,
      ticker: d.ticker,
      decision: d,
    });
  }

  for (const f of fills) {
    events.push({
      type: "fill",
      timestamp: f.timestamp,
      ticker: f.ticker,
      fill: f,
    });
  }

  events.sort(
    (a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
  );

  const groups: TimelineEvent[][] = [];
  let currentGroup: TimelineEvent[] = [];
  let currentTicker: string | null = null;
  let lastTime: number | null = null;

  for (const event of events) {
    const eventTime = new Date(event.timestamp).getTime();
    const isSameGroup =
      currentTicker === event.ticker &&
      lastTime !== null &&
      Math.abs(eventTime - lastTime) < 60000;

    if (isSameGroup) {
      currentGroup.push(event);
    } else {
      if (currentGroup.length > 0) {
        groups.push(currentGroup);
      }
      currentGroup = [event];
      currentTicker = event.ticker;
    }
    lastTime = eventTime;
  }

  if (currentGroup.length > 0) {
    groups.push(currentGroup);
  }

  return groups.slice(0, 50);
}

export function TransactionTimeline({
  decisions,
  recentFills,
  selectedIndex,
}: TransactionTimelineProps) {
  const colors = useColors();
  const groups = groupEvents(decisions, recentFills);

  return (
    <box style={{ flexDirection: "column", flexGrow: 1 }}>
      <Panel title="transaction timeline" flexGrow={1}>
        {groups.length === 0 ? (
          <box style={{ flexDirection: "column", gap: 1 }}>
            <text fg={colors.textDim}>no transactions yet...</text>
            <text fg={colors.textDim}>
              decisions and fills will appear here as the engine runs
            </text>
          </box>
        ) : (
          <box style={{ flexDirection: "column", gap: 0 }}>
            {groups.map((group, groupIdx) => (
              <box key={groupIdx} style={{ flexDirection: "column" }}>
                {group.map((event, eventIdx) => {
                  const isSelected =
                    selectedIndex === groupIdx && eventIdx === 0;

                  if (event.type === "decision" && event.decision) {
                    const d = event.decision;
                    const actionColor =
                      d.action === "enter"
                        ? colors.success
                        : d.action === "exit"
                          ? colors.warning
                          : colors.textDim;

                    return (
                      <box
                        key={`d-${d.id}`}
                        style={{
                          flexDirection: "row",
                          backgroundColor: isSelected
                            ? colors.bgAlt
                            : undefined,
                        }}
                      >
                        <text style={{ width: 10 }} fg={colors.textDim}>
                          {formatTime(d.timestamp)}
                        </text>
                        <text style={{ width: 3 }} fg={actionColor}>
                          ●
                        </text>
                        <text style={{ width: 8 }} fg={actionColor}>
                          DECIDE
                        </text>
                        <text style={{ width: 20 }} fg={colors.accent}>
                          {d.ticker.slice(0, 18)}
                        </text>
                        <text style={{ width: 8 }} fg={actionColor}>
                          {d.action}
                        </text>
                        <text
                          style={{ width: 6 }}
                          fg={
                            d.side === "Yes"
                              ? colors.success
                              : d.side === "No"
                                ? colors.error
                                : colors.textDim
                          }
                        >
                          {d.side || "-"}
                        </text>
                        <text fg={colors.textDim}>
                          score={d.score.toFixed(2)}
                        </text>
                      </box>
                    );
                  }

                  if (event.type === "fill" && event.fill) {
                    const f = event.fill;
                    const pnl = f.pnl;
                    const hasPnl = pnl != null && typeof pnl === "number";
                    const pnlColor =
                      hasPnl && pnl >= 0 ? colors.success : colors.error;

                    return (
                      <box
                        key={`f-${f.ticker}-${f.timestamp}`}
                        style={{
                          flexDirection: "row",
                          backgroundColor: isSelected
                            ? colors.bgAlt
                            : undefined,
                        }}
                      >
                        <text style={{ width: 10 }} fg={colors.textDim}>
                          {formatTime(f.timestamp)}
                        </text>
                        <text style={{ width: 3 }} fg={colors.accent}>
                          ◉
                        </text>
                        <text style={{ width: 8 }} fg={colors.accent}>
                          FILL
                        </text>
                        <text style={{ width: 20 }} fg={colors.accent}>
                          {f.ticker.slice(0, 18)}
                        </text>
                        <text style={{ width: 8 }} fg={colors.text}>
                          {f.quantity}
                        </text>
                        <text style={{ width: 6 }} fg={colors.text}>
                          @{f.price.toFixed(2)}
                        </text>
                        {hasPnl && (
                          <text fg={pnlColor}>
                            {pnl >= 0 ? "+" : ""}${pnl.toFixed(2)}
                          </text>
                        )}
                        {f.exitReason && (
                          <text fg={colors.textDim}> ({f.exitReason})</text>
                        )}
                      </box>
                    );
                  }

                  return null;
                })}
                {groupIdx < groups.length - 1 && (
                  <box style={{ height: 1 }}>
                    <text fg={colors.textDim}> </text>
                  </box>
                )}
              </box>
            ))}
          </box>
        )}
      </Panel>

      <Panel title="legend" marginTop={1}>
        <box style={{ flexDirection: "row", gap: 3 }}>
          <text>
            <span fg={colors.success}>● DECIDE</span>
            <span fg={colors.textDim}> enter</span>
          </text>
          <text>
            <span fg={colors.warning}>● DECIDE</span>
            <span fg={colors.textDim}> exit</span>
          </text>
          <text>
            <span fg={colors.textDim}>● DECIDE</span>
            <span fg={colors.textDim}> skip</span>
          </text>
          <text>
            <span fg={colors.accent}>◉ FILL</span>
            <span fg={colors.textDim}> executed</span>
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
