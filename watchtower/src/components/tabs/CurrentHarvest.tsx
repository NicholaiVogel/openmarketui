import { useColors, useGardenStore, useTerminalSize } from "../../hooks";
import { Panel } from "../shared/Panel";
import type { Position } from "../../types";

interface CurrentHarvestProps {
  positions: Position[];
  selectedIndex: number;
}

function parseTicker(ticker: string): { symbol: string; date: string | null } {
  const parts = ticker.split("-");
  if (parts.length >= 2) {
    const symbol = (parts[0] ?? "").replace(/^KX/, "");
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

function formatHoursHeld(hours: number | undefined | null): string {
  if (hours == null || isNaN(hours)) return "-";
  if (hours < 1) return "<1h";
  if (hours < 24) return `${Math.floor(hours)}h`;
  const days = Math.floor(hours / 24);
  const remainingHours = Math.floor(hours % 24);
  if (remainingHours === 0) return `${days}d`;
  return `${days}d ${remainingHours}h`;
}

function formatEntryTime(timestamp: string | undefined | null): string {
  if (!timestamp) return "-";
  const date = new Date(timestamp);
  if (isNaN(date.getTime())) return "-";

  const now = new Date();
  const isToday = date.toDateString() === now.toDateString();
  const time = date.toLocaleTimeString("en-US", {
    hour: "numeric",
    minute: "2-digit",
    hour12: true,
  });
  if (isToday) return `today ${time}`;
  return date.toLocaleDateString("en-US", { month: "short", day: "numeric" }) + ` ${time}`;
}

export function CurrentHarvest({
  positions,
  selectedIndex,
}: CurrentHarvestProps) {
  const colors = useColors();
  const viewMode = useGardenStore((s) => s.positionsViewMode);
  const { rows: terminalRows } = useTerminalSize();


  // Calculate visible items based on terminal height
  // Reserve space for: header(3) + panel border(2) + portfolio panel(5) + footer(3) + scroll indicators(2)
  const reservedRows = 15;
  const availableRows = Math.max(5, terminalRows - reservedRows);

  // Card = ~4 rows each (border + 3 lines content), List = 1 row each
  const rowsPerItem = viewMode === "card" ? 5 : 1;
  const visibleCount = Math.max(1, Math.floor(availableRows / rowsPerItem));
  const halfVisible = Math.floor(visibleCount / 2);
  let startIdx = Math.max(0, selectedIndex - halfVisible);
  const endIdx = Math.min(positions.length, startIdx + visibleCount);
  if (endIdx - startIdx < visibleCount) {
    startIdx = Math.max(0, endIdx - visibleCount);
  }
  const visiblePositions = positions.slice(startIdx, endIdx);


  return (
    <box style={{ flexDirection: "column", flexGrow: 1 }}>
      <Panel title={`active positions (${positions.length})`} flexGrow={1}>
        {positions.length === 0 ? (
          <text fg={colors.textDim}>
            no open positions
          </text>
        ) : (
          <box style={{ flexDirection: "column", gap: 0 }}>
            {startIdx > 0 && (
              <text fg={colors.textDim}>↑ {startIdx} more above</text>
            )}

            {viewMode === "card" ? (
              // Card view
              <box style={{ flexDirection: "column", gap: 1 }}>
                {visiblePositions.map((pos, visibleIdx) => {
                  const actualIdx = startIdx + visibleIdx;
                  const isSelected = selectedIndex === actualIdx;
                  return (
                    <PositionCard
                      key={pos.ticker}
                      position={pos}
                      isSelected={isSelected}
                      colors={colors}
                    />
                  );
                })}
              </box>
            ) : (
              // List view
              <box style={{ flexDirection: "column", gap: 0 }}>
                {visiblePositions.map((pos, visibleIdx) => {
                  const actualIdx = startIdx + visibleIdx;
                  const isSelected = selectedIndex === actualIdx;
                  return (
                    <PositionRow
                      key={pos.ticker}
                      position={pos}
                      isSelected={isSelected}
                      colors={colors}
                    />
                  );
                })}
              </box>
            )}

            {endIdx < positions.length && (
              <text fg={colors.textDim}>↓ {positions.length - endIdx} more below</text>
            )}
          </box>
        )}
      </Panel>

      <box style={{ marginTop: 1, flexDirection: "row", gap: 2, paddingLeft: 1 }}>
        <text fg={colors.textDim}>[j/k] scroll</text>
        <text fg={colors.textDim}>[enter] drilldown</text>
        <text fg={colors.accent}>
          [v] {viewMode === "card" ? "list" : "card"} view
        </text>
      </box>
    </box>
  );
}

// Card component for spacious view
function PositionCard({
  position: pos,
  isSelected,
  colors,
}: {
  position: Position;
  isSelected: boolean;
  colors: ReturnType<typeof useColors>;
}) {
  const entryPrice = pos.entryPrice ?? 0;
  const qty = pos.quantity ?? 0;
  const cost = entryPrice * qty;
  const unrealizedPnl = pos.unrealizedPnl ?? 0;
  const pnlPct = pos.pnlPct ?? 0;
  const pnlColor = unrealizedPnl >= 0 ? colors.success : colors.error;
  const currentPrice = pos.currentPrice ?? entryPrice;

  const shortTicker = pos.ticker.length > 30 ? pos.ticker.slice(0, 30) + "…" : pos.ticker;
  const pnlStr = `${unrealizedPnl >= 0 ? "+" : ""}$${unrealizedPnl.toFixed(2)} (${pnlPct >= 0 ? "+" : ""}${pnlPct.toFixed(1)}%)`;

  if (isSelected) {
    // Expanded view for selected
    const fullTitle = pos.title || "-";
    return (
      <box
        style={{
          borderStyle: "rounded",
          borderColor: colors.accent,
          paddingLeft: 1,
          paddingRight: 1,
        }}
      >
        <text fg={colors.accent}>
          {"▸ "}{pos.ticker}
          {"\n"}
          <span fg={colors.text}>{"  "}{fullTitle}</span>
          {"\n"}
          <span fg={pos.side === "Yes" ? colors.success : colors.error}>{"  "}{pos.side}</span>
          <span fg={colors.text}> · {qty} contracts @ {(entryPrice * 100).toFixed(0)}¢</span>
          {currentPrice !== entryPrice && currentPrice > 0 && (
            <span fg={colors.textDim}> → {(currentPrice * 100).toFixed(0)}¢</span>
          )}
          {"\n"}
          <span fg={colors.textDim}>{"  "}Cost: </span>
          <span fg={colors.text}>${cost.toFixed(2)}</span>
          <span fg={colors.textDim}> · P&L: </span>
          <span fg={pnlColor}>{pnlStr}</span>
          {"\n"}
          <span fg={colors.textDim}>{"  "}Held: {formatHoursHeld(pos.hoursHeld)}</span>
          <span fg={colors.textDim}> · Entry: {formatEntryTime(pos.entryTime)}</span>
          {pos.category && <span fg={colors.textDim}> · {pos.category}</span>}
        </text>
      </box>
    );
  }

  // Compact view for non-selected
  const shortTitle = pos.title ? (pos.title.length > 55 ? pos.title.slice(0, 55) + "..." : pos.title) : "";
  return (
    <box
      style={{
        borderStyle: "rounded",
        borderColor: colors.bgAlt,
        paddingLeft: 1,
        paddingRight: 1,
      }}
    >
      <text fg={colors.text}>
        {"  "}{shortTicker}
        {"\n"}
        <span fg={colors.textDim}>{"  "}{shortTitle}</span>
        {"\n"}
        <span fg={pos.side === "Yes" ? colors.success : colors.error}>{"  "}{pos.side}</span>
        <span fg={colors.textDim}> · {qty} @ {(entryPrice * 100).toFixed(0)}¢ · </span>
        <span fg={pnlColor}>{pnlStr}</span>
      </text>
    </box>
  );
}

// List row with expanded mode for selected
function PositionRow({
  position: pos,
  isSelected,
  colors,
}: {
  position: Position;
  isSelected: boolean;
  colors: ReturnType<typeof useColors>;
}) {
  const { symbol } = parseTicker(pos.ticker);
  const entryPrice = pos.entryPrice ?? 0;
  const qty = pos.quantity ?? 0;
  const cost = entryPrice * qty;
  const unrealizedPnl = pos.unrealizedPnl ?? 0;
  const pnlPct = pos.pnlPct ?? 0;
  const pnlColor = unrealizedPnl >= 0 ? colors.success : colors.error;
  const currentPrice = pos.currentPrice ?? entryPrice;

  if (isSelected) {
    // Expanded view
    const pnlStr = `${unrealizedPnl >= 0 ? "+" : ""}$${unrealizedPnl.toFixed(2)} (${pnlPct >= 0 ? "+" : ""}${pnlPct.toFixed(1)}%)`;
    return (
      <box style={{ backgroundColor: colors.bgAlt, paddingLeft: 1, paddingRight: 1 }}>
        <text fg={colors.accent}>
          {"▸ "}{pos.ticker}
          {"\n"}
          <span fg={colors.text}>{"  "}{pos.title || "-"}</span>
          {"\n"}
          <span fg={pos.side === "Yes" ? colors.success : colors.error}>{"  "}{pos.side}</span>
          <span fg={colors.text}> · {qty} @ {(entryPrice * 100).toFixed(0)}¢</span>
          {currentPrice !== entryPrice && currentPrice > 0 && (
            <span fg={colors.textDim}> → {(currentPrice * 100).toFixed(0)}¢</span>
          )}
          <span fg={colors.textDim}> · ${cost.toFixed(2)} · </span>
          <span fg={pnlColor}>{pnlStr}</span>
          <span fg={colors.textDim}> · {formatHoursHeld(pos.hoursHeld)}</span>
          {pos.category && <span fg={colors.textDim}> · {pos.category}</span>}
        </text>
      </box>
    );
  }

  // Compact single line
  return (
    <box style={{ paddingLeft: 1 }}>
      <text>
        <span fg={colors.textDim}>{"  "}</span>
        <span fg={pos.side === "Yes" ? colors.success : colors.error}>{pos.side.padEnd(4)}</span>
        <span fg={colors.text}>{symbol.slice(0, 14).padEnd(15)}</span>
        <span fg={colors.text}>{String(qty).padStart(5)}x </span>
        <span fg={colors.textDim}>@{(entryPrice * 100).toFixed(0).padStart(3)}¢ </span>
        <span fg={colors.text}>${cost.toFixed(2).padStart(7)} </span>
        <span fg={pnlColor}>{(unrealizedPnl >= 0 ? "+" : "") + unrealizedPnl.toFixed(2).padStart(6)} </span>
        <span fg={pnlColor}>{(pnlPct >= 0 ? "+" : "") + pnlPct.toFixed(0).padStart(3)}% </span>
        <span fg={colors.textDim}>{formatHoursHeld(pos.hoursHeld)}</span>
      </text>
    </box>
  );
}
