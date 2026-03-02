import { useColors, useModeStore, useGardenStore } from "../../hooks";
import type { TradingMode } from "../../types";

const MODE_OPTIONS: Array<{
  id: TradingMode;
  label: string;
  hint: string;
}> = [
  { id: "paper", label: "Paper Trading", hint: "simulated trades, real data" },
  { id: "backtest", label: "Backtest", hint: "test on historical data" },
  { id: "live", label: "Live Trading", hint: "coming soon" },
];

function formatPnl(value: number): string {
  const sign = value >= 0 ? "+" : "";
  return `${sign}$${Math.abs(value).toFixed(2)}`;
}

function formatPct(value: number): string {
  const sign = value >= 0 ? "+" : "";
  return `${sign}${value.toFixed(1)}%`;
}

export function ModeSelector() {
  const colors = useColors();
  const { portfolio } = useGardenStore();
  const {
    viewMode,
    sessionStatus,
    activeConfig,
    menuIndex,
  } = useModeStore();

  const isSessionActive = sessionStatus === "running" && viewMode !== "idle";
  const totalPnl = portfolio?.totalPnl ?? 0;
  const returnPct = portfolio?.returnPct ?? 0;
  const pnlColor = totalPnl >= 0 ? colors.success : colors.error;

  return (
    <box style={{ flexDirection: "column" }}>
      {isSessionActive && (
        <box style={{ flexDirection: "column", marginBottom: 1 }}>
          <text fg={colors.success}>
            active: {viewMode.toUpperCase()} ● running
          </text>
          <text>
            <span fg={colors.text}>P&L: </span>
            <span fg={pnlColor}>
              {formatPnl(totalPnl)} ({formatPct(returnPct)})
            </span>
          </text>
          <text fg={colors.textDim}>
            config: {activeConfig?.name || "default"}
          </text>
          <box style={{ marginTop: 1 }}>
            <text fg={colors.warning}>[enter] stop session</text>
          </box>
          <text fg={colors.textDim}>---</text>
        </box>
      )}

      <text fg={colors.textDim} style={{ marginBottom: 1 }}>
        {isSessionActive ? "switch mode (stops current):" : "select trading mode:"}
      </text>

      {MODE_OPTIONS.map((option, idx) => {
        const isSelected = idx === menuIndex;
        const isActive = viewMode === option.id;
        const isRunning = isActive && sessionStatus === "running";
        const isDisabled = option.id === "live";

        return (
          <box
            key={option.id}
            style={{
              flexDirection: "row",
              backgroundColor: isSelected ? colors.bgAlt : undefined,
            }}
          >
            <text
              fg={
                isDisabled
                  ? colors.textDim
                  : isSelected
                    ? colors.accent
                    : colors.text
              }
            >
              {isSelected ? "> " : "  "}
              {option.label}
            </text>
            <text fg={isRunning ? colors.success : colors.textDim}>
              {" "}
              {isRunning ? "● running" : isActive ? "○ selected" : ""}
            </text>
          </box>
        );
      })}

      <box style={{ marginTop: 1 }}>
        <text fg={colors.textDim}>
          config: {activeConfig?.name || "none"} (${activeConfig?.initialCapital?.toLocaleString() || 0})
        </text>
      </box>

      <box style={{ flexDirection: "row", gap: 2, marginTop: 1 }}>
        <text fg={colors.textDim}>[c] config</text>
        <text fg={colors.textDim}>[p] presets</text>
        <text fg={colors.textDim}>[d] data</text>
        <text fg={colors.textDim}>[h] back</text>
      </box>
    </box>
  );
}
