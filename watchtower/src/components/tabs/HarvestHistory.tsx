import { useColors, useTerminalSize } from "../../hooks";
import { Panel } from "../shared/Panel";
import type { Fill } from "../../types";

interface HarvestHistoryProps {
  fills: Fill[];
  selectedIndex: number;
}

function formatRelativeTime(timestamp: string): string {
  const now = new Date();
  const then = new Date(timestamp);
  const diffMs = now.getTime() - then.getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffDays > 0) return `${diffDays}d ago`;
  if (diffHours > 0) return `${diffHours}h ago`;
  if (diffMins > 0) return `${diffMins}m ago`;
  return "just now";
}

function formatTime(timestamp: string): string {
  const date = new Date(timestamp);
  return date.toLocaleTimeString("en-US", {
    hour: "numeric",
    minute: "2-digit",
    hour12: true,
  });
}

function formatDate(timestamp: string): string {
  const date = new Date(timestamp);
  return date.toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
  });
}

function parseTicker(ticker: string): { symbol: string; date: string | null } {
  // Kalshi tickers often have format like KXNBABLK-26FEB03AT
  // Try to extract meaningful parts
  const parts = ticker.split("-");
  if (parts.length >= 2) {
    const symbol = (parts[0] ?? "").replace(/^KX/, "");
    // Try to parse date from second part (e.g., 26FEB03AT -> Feb 26)
    const datePart = parts[1] ?? "";
    const dateMatch = datePart.match(/^(\d{1,2})([A-Z]{3})(\d{2})/);
    if (dateMatch && dateMatch[1] && dateMatch[2]) {
      const day = dateMatch[1];
      const month = dateMatch[2];
      return { symbol, date: `${month} ${day}` };
    }
    return { symbol, date: null };
  }
  return { symbol: ticker.slice(0, 12), date: null };
}

export function HarvestHistory({ fills, selectedIndex }: HarvestHistoryProps) {
  const colors = useColors();
  const { rows: terminalRows } = useTerminalSize();

  // Calculate visible items based on terminal height
  // Reserve space for: header(3) + panel border(2) + summary panel(4) + footer(3) + scroll indicators(2)
  const reservedRows = 14;
  const availableRows = Math.max(5, terminalRows - reservedRows);

  // Each fill row = 1 line (2 when selected with detail)
  const visibleCount = Math.max(3, availableRows);
  const halfVisible = Math.floor(visibleCount / 2);
  let startIdx = Math.max(0, selectedIndex - halfVisible);
  const endIdx = Math.min(fills.length, startIdx + visibleCount);
  if (endIdx - startIdx < visibleCount) {
    startIdx = Math.max(0, endIdx - visibleCount);
  }
  const visibleFills = fills.slice(startIdx, endIdx);

  // Group fills by date for display
  const today = new Date().toDateString();
  const yesterday = new Date(Date.now() - 86400000).toDateString();

  function getDateLabel(timestamp: string): string {
    const date = new Date(timestamp).toDateString();
    if (date === today) return "today";
    if (date === yesterday) return "yesterday";
    return formatDate(timestamp);
  }

  return (
    <box style={{ flexDirection: "column", flexGrow: 1 }}>
      <Panel title={`fills (${fills.length} total)`} flexGrow={1}>
        {fills.length === 0 ? (
          <text fg={colors.textDim}>no trades recorded yet...</text>
        ) : (
          <box style={{ flexDirection: "column", gap: 0 }}>
            {/* scroll indicator */}
            {startIdx > 0 && (
              <text fg={colors.textDim}>
                ↑ {startIdx} more above
              </text>
            )}

            {visibleFills.map((fill, visibleIdx) => {
              const actualIdx = startIdx + visibleIdx;
              const isSelected = selectedIndex === actualIdx;
              const isEntry = fill.pnl == null;
              const isExit = !isEntry;
              const { symbol, date: tickerDate } = parseTicker(fill.ticker);

              const price = fill.price ?? 0;
              const qty = fill.quantity ?? 0;
              const cost = price * qty;
              const pnl = fill.pnl ?? 0;
              const fee = fill.fee ?? 0;

              // Color based on trade type
              const rowBg = isSelected ? colors.bgAlt : undefined;
              const typeColor = isExit
                ? pnl >= 0
                  ? colors.success
                  : colors.error
                : colors.accent;

              return (
                <box
                  key={`${fill.ticker}-${actualIdx}`}
                  style={{
                    flexDirection: "column",
                    backgroundColor: rowBg,
                    paddingLeft: 1,
                    paddingRight: 1,
                    marginBottom: isSelected ? 0 : 0,
                  }}
                >
                  {/* Main row */}
                  <box style={{ flexDirection: "row" }}>
                    <text style={{ width: 2 }} fg={typeColor}>
                      {isSelected ? "▸" : " "}
                    </text>
                    <text style={{ width: 5 }} fg={typeColor}>
                      {isEntry ? "BUY" : "SELL"}
                    </text>
                    <text style={{ width: 5 }} fg={fill.side === "Yes" ? colors.success : colors.error}>
                      {fill.side}
                    </text>
                    <text style={{ width: 14 }} fg={colors.text}>
                      {symbol.slice(0, 12)}
                    </text>
                    <text style={{ width: 7 }} fg={colors.text}>
                      {qty}x
                    </text>
                    <text style={{ width: 7 }} fg={colors.textDim}>
                      @{(price * 100).toFixed(0)}¢
                    </text>
                    <text style={{ width: 9 }} fg={colors.text}>
                      ${cost.toFixed(2)}
                    </text>
                    {isExit ? (
                      <text style={{ width: 10 }} fg={pnl >= 0 ? colors.success : colors.error}>
                        {pnl >= 0 ? "+" : ""}{pnl.toFixed(2)}
                      </text>
                    ) : (
                      <text style={{ width: 10 }} fg={colors.textDim}>
                        -
                      </text>
                    )}
                    <text style={{ width: 8 }} fg={colors.textDim}>
                      {formatRelativeTime(fill.timestamp)}
                    </text>
                  </box>

                  {/* Detail row for selected item */}
                  {isSelected && (
                    <box style={{ flexDirection: "row", paddingLeft: 2 }}>
                      <text fg={colors.textDim}>
                        {fill.ticker}
                        {tickerDate && ` (${tickerDate})`}
                        {" · "}
                        {formatTime(fill.timestamp)} {getDateLabel(fill.timestamp)}
                        {fee > 0 && ` · fee: $${fee.toFixed(2)}`}
                        {fill.exitReason && ` · ${fill.exitReason}`}
                      </text>
                    </box>
                  )}
                </box>
              );
            })}

            {/* scroll indicator */}
            {endIdx < fills.length && (
              <text fg={colors.textDim}>
                ↓ {fills.length - endIdx} more below
              </text>
            )}
          </box>
        )}
      </Panel>

      <Panel title="summary" marginTop={1}>
        <box style={{ flexDirection: "row", gap: 3 }}>
          <text>
            <span fg={colors.textDim}>entries: </span>
            <span fg={colors.accent}>{fills.filter(f => f.pnl == null).length}</span>
          </text>
          <text>
            <span fg={colors.textDim}>exits: </span>
            <span fg={colors.text}>{fills.filter(f => f.pnl != null).length}</span>
          </text>
          <text>
            <span fg={colors.textDim}>realized P&L: </span>
            <span fg={fills.reduce((s, f) => s + (f.pnl ?? 0), 0) >= 0 ? colors.success : colors.error}>
              ${fills.reduce((s, f) => s + (f.pnl ?? 0), 0).toFixed(2)}
            </span>
          </text>
          <text>
            <span fg={colors.textDim}>fees: </span>
            <span fg={colors.warning}>
              ${fills.reduce((s, f) => s + (f.fee ?? 0), 0).toFixed(2)}
            </span>
          </text>
          <text fg={colors.textDim}>[j/k] scroll</text>
        </box>
      </Panel>
    </box>
  );
}
