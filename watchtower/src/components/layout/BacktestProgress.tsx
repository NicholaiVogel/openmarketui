import { useColors, useModeStore } from "../../hooks";
import { formatDuration, renderProgressBar } from "../../utils/format";

export function BacktestProgress() {
  const colors = useColors();
  const { backtestProgress } = useModeStore();

  if (backtestProgress.status === "idle") {
    return null;
  }

  const progressPct = backtestProgress.progressPct ?? 0;
  const elapsed = backtestProgress.elapsedSecs ?? 0;

  return (
    <box
      style={{
        border: true,
        borderColor: colors.border,
        backgroundColor: colors.bg,
        padding: 1,
        flexDirection: "column",
      }}
      title=" backtest running "
    >
      <box style={{ flexDirection: "row" }}>
        <text fg={colors.textDim}>phase: </text>
        <text fg={colors.text}>{backtestProgress.phase || "initializing"}</text>
      </box>

      <box style={{ flexDirection: "row", marginTop: 1 }}>
        <text fg={colors.textDim}>progress: </text>
        <text fg={colors.accent}>
          {renderProgressBar(progressPct, 30)} {progressPct.toFixed(0)}%
        </text>
      </box>

      <box style={{ flexDirection: "row", marginTop: 1 }}>
        <text fg={colors.textDim}>elapsed: </text>
        <text fg={colors.text}>{formatDuration(elapsed)}</text>
      </box>

      {backtestProgress.error && (
        <box style={{ marginTop: 1 }}>
          <text fg={colors.error}>error: {backtestProgress.error}</text>
        </box>
      )}

      <box style={{ marginTop: 1 }}>
        <text fg={colors.textDim}>[esc] cancel</text>
      </box>
    </box>
  );
}

export function BacktestResultsPanel() {
  const colors = useColors();
  const { backtestResult } = useModeStore();

  if (!backtestResult) {
    return <text fg={colors.textDim}>no backtest results</text>;
  }

  const returnColor =
    backtestResult.totalReturnPct >= 0 ? colors.success : colors.error;

  return (
    <box style={{ flexDirection: "column" }}>
      <text fg={colors.accent}>
        backtest results
      </text>

      <box style={{ marginTop: 1 }}>
        <text fg={colors.textDim}>total return: </text>
        <text fg={returnColor}>{backtestResult.totalReturnPct.toFixed(2)}%</text>
      </box>

      <box>
        <text fg={colors.textDim}>sharpe ratio: </text>
        <text fg={colors.text}>{backtestResult.sharpeRatio.toFixed(3)}</text>
      </box>

      <box>
        <text fg={colors.textDim}>max drawdown: </text>
        <text fg={colors.error}>{backtestResult.maxDrawdownPct.toFixed(2)}%</text>
      </box>

      <box>
        <text fg={colors.textDim}>win rate: </text>
        <text fg={colors.text}>{backtestResult.winRate.toFixed(1)}%</text>
      </box>

      <box>
        <text fg={colors.textDim}>total trades: </text>
        <text fg={colors.text}>{backtestResult.totalTrades}</text>
      </box>
    </box>
  );
}
