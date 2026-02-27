import { useColors, useModeStore } from "../../hooks";
import { Panel } from "../shared/Panel";
import { formatDuration, renderProgressBar } from "../../utils/format";
import type { EngineStatus } from "../../types";

interface SessionControlStripProps {
  engineStatus: EngineStatus | null;
}

function modeBadgeColor(
  mode: string,
  colors: ReturnType<typeof useColors>,
): string {
  if (mode === "paper") return colors.warning;
  if (mode === "backtest") return colors.accent;
  if (mode === "live") return colors.error;
  return colors.textDim;
}

export function SessionControlStrip({ engineStatus }: SessionControlStripProps) {
  const colors = useColors();
  const {
    viewMode,
    sessionStatus,
    sessionError,
    activeConfig,
    backtestProgress,
    backtestResult,
  } = useModeStore();

  const modeLabel = viewMode === "idle" ? "IDLE" : viewMode.toUpperCase();
  const statusDot =
    sessionStatus === "running"
      ? "\u25CF"
      : sessionStatus === "paused"
        ? "\u25D4"
        : "\u25CB";
  const statusLabel = sessionStatus === "idle" ? "stopped" : sessionStatus;
  const statusColor =
    sessionStatus === "running"
      ? colors.success
      : sessionStatus === "paused"
        ? colors.warning
        : sessionStatus === "error"
          ? colors.error
          : colors.textDim;

  const presetName = activeConfig?.name ?? "none";
  const capital = activeConfig?.initialCapital ?? 0;

  const engineState = engineStatus?.state ?? "—";
  const ticks = engineStatus?.ticksCompleted ?? 0;
  const uptime = engineStatus?.uptimeSecs ?? 0;

  const isBacktestRunning = backtestProgress.status === "running";
  const isBacktestComplete = backtestProgress.status === "complete";
  const isBacktestFailed = backtestProgress.status === "failed";
  const isIdle = sessionStatus === "idle" && viewMode === "idle";
  const isRunning = sessionStatus === "running" || sessionStatus === "paused";

  return (
    <Panel title="session" flexGrow={0}>
      <box style={{ flexDirection: "column", width: 34 }}>
        {/* mode + status */}
        <box style={{ flexDirection: "row" }}>
          <text fg={modeBadgeColor(viewMode, colors)}>
            {modeLabel}
          </text>
          <text fg={statusColor}> {statusDot} {statusLabel}</text>
        </box>

        {/* preset info */}
        <box style={{ flexDirection: "row", marginTop: 1 }}>
          <text fg={colors.textDim}>preset: </text>
          <text fg={colors.text}>
            {presetName} (${capital.toLocaleString()})
          </text>
        </box>

        {/* engine status line */}
        {isRunning && !isBacktestRunning && (
          <box style={{ flexDirection: "row" }}>
            <text fg={colors.textDim}>engine: </text>
            <text fg={colors.text}>
              {engineState} ticks: {ticks}
            </text>
            {uptime > 0 && (
              <text fg={colors.textDim}> ({formatDuration(uptime)})</text>
            )}
          </box>
        )}

        {/* keybinding hints */}
        <box style={{ flexDirection: "column", marginTop: 1 }}>
          {isIdle && (
            <>
              <text fg={colors.textDim}>
                [b] backtest [p] paper
              </text>
              <text fg={colors.textDim}>
                {"[</>] preset  [c] config"}
              </text>
            </>
          )}
          {isRunning && !isBacktestRunning && (
            <text fg={colors.textDim}>
              [x] stop [space] pause
            </text>
          )}
          {isBacktestRunning && (
            <text fg={colors.textDim}>
              [x] stop [+/-] speed
            </text>
          )}
        </box>

        {/* inline backtest progress */}
        {isBacktestRunning && (
          <box style={{ flexDirection: "column", marginTop: 1 }}>
            <text fg={colors.accent}>
              {renderProgressBar(backtestProgress.progressPct ?? 0, 20)}{" "}
              {(backtestProgress.progressPct ?? 0).toFixed(0)}%{" "}
              {backtestProgress.phase || ""}
            </text>
            <box style={{ flexDirection: "row" }}>
              <text fg={colors.textDim}>elapsed: </text>
              <text fg={colors.text}>
                {formatDuration(backtestProgress.elapsedSecs ?? 0)}
              </text>
            </box>
            <box style={{ flexDirection: "row" }}>
              <text fg={colors.textDim}>speed: </text>
              <text fg={colors.text}>
                {activeConfig?.backtestIntervalHours ?? 1}h
              </text>
            </box>
            <box style={{ flexDirection: "row" }}>
              <text fg={colors.textDim}>open positions: </text>
              <text fg={colors.text}>
                {backtestProgress.liveSnapshot?.open_positions ?? 0}
              </text>
            </box>
            <box style={{ flexDirection: "row" }}>
              <text fg={colors.textDim}>fills/step: </text>
              <text fg={colors.text}>
                {backtestProgress.liveSnapshot?.fills_this_step ?? 0}
              </text>
            </box>
          </box>
        )}

        {/* inline backtest results */}
        {isBacktestComplete && backtestResult && (
          <box style={{ flexDirection: "column", marginTop: 1 }}>
            <text fg={colors.accent}>results</text>
            <box style={{ flexDirection: "row" }}>
              <text fg={colors.textDim}>return: </text>
              <text
                fg={
                  backtestResult.totalReturnPct >= 0
                    ? colors.success
                    : colors.error
                }
              >
                {backtestResult.totalReturnPct.toFixed(2)}%
              </text>
            </box>
            <box style={{ flexDirection: "row" }}>
              <text fg={colors.textDim}>sharpe: </text>
              <text fg={colors.text}>
                {backtestResult.sharpeRatio.toFixed(3)}
              </text>
            </box>
            <box style={{ flexDirection: "row" }}>
              <text fg={colors.textDim}>drawdown: </text>
              <text fg={colors.error}>
                {backtestResult.maxDrawdownPct.toFixed(2)}%
              </text>
            </box>
            <box style={{ flexDirection: "row" }}>
              <text fg={colors.textDim}>win rate: </text>
              <text fg={colors.text}>
                {backtestResult.winRate.toFixed(1)}%
              </text>
            </box>
            <box style={{ flexDirection: "row" }}>
              <text fg={colors.textDim}>trades: </text>
              <text fg={colors.text}>{backtestResult.totalTrades}</text>
            </box>
            <text fg={colors.textDim} style={{ marginTop: 1 }}>
              [esc] dismiss
            </text>
          </box>
        )}

        {/* inline error */}
        {isBacktestFailed && (
          <box style={{ flexDirection: "column", marginTop: 1 }}>
            <text fg={colors.error}>
              {backtestProgress.error || "unknown error"}
            </text>
            <text fg={colors.textDim} style={{ marginTop: 1 }}>
              [esc] dismiss
            </text>
          </box>
        )}

        {/* session error (non-backtest) */}
        {sessionError && !isBacktestFailed && (
          <box style={{ marginTop: 1 }}>
            <text fg={colors.error}>{sessionError}</text>
          </box>
        )}
      </box>
    </Panel>
  );
}
