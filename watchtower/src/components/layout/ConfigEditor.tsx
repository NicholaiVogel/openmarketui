import { useColors, useModeStore, EDIT_FIELDS } from "../../hooks";

const FIELD_TOOLTIPS: Record<string, string> = {
  name: "Label for this preset in the selector.",
  initialCapital: "Starting bankroll used for paper and backtest runs.",
  maxPositions: "Maximum number of simultaneous open positions.",
  kellyFraction: "Sizing aggressiveness from 0 to 1; higher means larger entries.",
  maxPositionPct:
    "Per-position cap as a fraction of capital. In backtests this also sets max contracts.",
  takeProfitPct:
    "Exit threshold for gains (0.50 means close at +50% from entry).",
  stopLossPct:
    "Exit threshold for losses (0.25 means close at -25% from entry).",
  maxHoldHours: "Force-close a position after this many hours if still open.",
  minTimeToCloseHours:
    "Paper-mode filter: ignore markets closing sooner than this.",
  maxTimeToCloseHours:
    "Paper-mode filter: ignore markets closing later than this horizon.",
  cashReservePct: "Paper mode only: keep this share of cash unallocated.",
  maxEntriesPerTick: "Paper mode only: cap new entries each engine cycle.",
  backtestDateRange:
    "Historical window to simulate. Shorter windows run faster but are less robust.",
  backtestIntervalHours:
    "Backtest step size. Lower values react faster but take longer to compute.",
};

function formatValue(value: unknown, type: string): string {
  if (type === "currency") {
    return `$${(value as number).toLocaleString()}`;
  }
  if (type === "percent") {
    return `${((value as number) * 100).toFixed(0)}%`;
  }
  if (type === "decimal") {
    return (value as number).toFixed(2);
  }
  return String(value);
}

export function ConfigEditor() {
  const colors = useColors();
  const {
    viewMode,
    editingConfig,
    editFieldIndex,
    backtestProgress,
  } = useModeStore();

  if (!editingConfig) {
    return <text fg={colors.textDim}>no config to edit</text>;
  }

  const showBacktestFields = viewMode === "backtest";
  const isBacktestRunning = backtestProgress.status === "running";
  const isBacktestComplete = backtestProgress.status === "complete";
  const hasDates = editingConfig.backtestStart && editingConfig.backtestEnd;
  const selectedFieldKey =
    editFieldIndex < EDIT_FIELDS.length
      ? EDIT_FIELDS[editFieldIndex]?.key
      : editFieldIndex === EDIT_FIELDS.length
        ? "backtestDateRange"
        : editFieldIndex === EDIT_FIELDS.length + 1
          ? "backtestIntervalHours"
          : undefined;
  const selectedTooltip = selectedFieldKey ? FIELD_TOOLTIPS[selectedFieldKey] : undefined;

  return (
    <box style={{ flexDirection: "column" }}>
      <text fg={colors.textDim} style={{ marginBottom: 1 }}>
        edit trading config (j/k, +/- to adjust, enter to save)
      </text>

      {EDIT_FIELDS.map((field, idx) => {
        const isSelected = idx === editFieldIndex;
        const value = (editingConfig as unknown as Record<string, unknown>)[field.key];

        return (
          <box
            key={field.key}
            style={{
              flexDirection: "row",
              justifyContent: "space-between",
              backgroundColor: isSelected ? colors.bgAlt : undefined,
              paddingLeft: 1,
              paddingRight: 1,
            }}
          >
            <text fg={isSelected ? colors.accent : colors.text}>
              {isSelected ? "> " : "  "}
              {field.label}:
            </text>
            <text fg={isSelected ? colors.accent : colors.text}>
              {formatValue(value, field.type)}
              {field.type !== "string" && isSelected ? " [+/-]" : ""}
            </text>
          </box>
        );
      })}

      {showBacktestFields && (
        <>
          <box
            style={{
              marginTop: 1,
              paddingTop: 1,
            }}
          >
            <text fg={colors.textDim}>--- backtest settings ---</text>
          </box>

          <box
            style={{
              flexDirection: "row",
              justifyContent: "space-between",
              backgroundColor:
                editFieldIndex === EDIT_FIELDS.length ? colors.bgAlt : undefined,
              paddingLeft: 1,
              paddingRight: 1,
            }}
          >
            <text
              fg={editFieldIndex === EDIT_FIELDS.length ? colors.accent : colors.text}
            >
              {editFieldIndex === EDIT_FIELDS.length ? "> " : "  "}
              date range:
            </text>
            <text fg={colors.text}>
              {editingConfig.backtestStart && editingConfig.backtestEnd
                ? `${editingConfig.backtestStart} to ${editingConfig.backtestEnd}`
                : "not set"}
              {editFieldIndex === EDIT_FIELDS.length ? " [d] change" : ""}
            </text>
          </box>

          <box
            style={{
              flexDirection: "row",
              justifyContent: "space-between",
              backgroundColor:
                editFieldIndex === EDIT_FIELDS.length + 1 ? colors.bgAlt : undefined,
              paddingLeft: 1,
              paddingRight: 1,
            }}
          >
            <text
              fg={
                editFieldIndex === EDIT_FIELDS.length + 1 ? colors.accent : colors.text
              }
            >
              {editFieldIndex === EDIT_FIELDS.length + 1 ? "> " : "  "}
              interval hours:
            </text>
            <text fg={colors.text}>
              {editingConfig.backtestIntervalHours || 1}
              {editFieldIndex === EDIT_FIELDS.length + 1 ? " [+/-]" : ""}
            </text>
          </box>
        </>
      )}

      {showBacktestFields && isBacktestRunning && (
        <box style={{ marginTop: 1 }}>
          <text fg={colors.warning}>
            backtest running: {backtestProgress.phase} ({backtestProgress.progressPct?.toFixed(0) || 0}%)
          </text>
        </box>
      )}

      {showBacktestFields && isBacktestComplete && (
        <box style={{ marginTop: 1 }}>
          <text fg={colors.success}>backtest complete</text>
        </box>
      )}

      {selectedTooltip && (
        <box style={{ marginTop: 1, paddingLeft: 1, paddingRight: 1 }}>
          <text fg={colors.textDim}>tip: {selectedTooltip}</text>
        </box>
      )}

      <box style={{ flexDirection: "row", gap: 2, marginTop: 1 }}>
        {showBacktestFields ? (
          isBacktestRunning ? (
            <text fg={colors.error}>[s] STOP BACKTEST</text>
          ) : hasDates ? (
            <text fg={colors.accent}>[s] START BACKTEST</text>
          ) : (
            <text fg={colors.textDim}>[s] select dates first</text>
          )
        ) : (
          <text fg={colors.textDim}>[enter] save</text>
        )}
        <text fg={colors.textDim}>[n] save as new</text>
        <text fg={colors.textDim}>[h] cancel</text>
      </box>
    </box>
  );
}
