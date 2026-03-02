import { useColors, useModeStore, useGardenStore } from "../../hooks";

export function Header() {
  const colors = useColors();
  const { viewMode, sessionStatus, sessionError, backtestProgress } = useModeStore();
  const { portfolio, positions } = useGardenStore();

  const formatPnl = (value: number) => {
    const sign = value >= 0 ? "+" : "";
    return `${sign}$${Math.abs(value).toFixed(2)}`;
  };

  const formatPct = (value: number) => {
    const sign = value >= 0 ? "+" : "";
    return `${sign}${value.toFixed(2)}%`;
  };

  const getModeDisplay = () => {
    if (sessionStatus === "error" && sessionError) {
      return { label: `ERROR: ${sessionError}`, color: colors.error, pnl: null };
    }
    if (viewMode === "idle") {
      return { label: "IDLE", color: colors.textDim, pnl: null };
    }
    if (viewMode === "backtest" && backtestProgress.status === "running") {
      const pct = backtestProgress.progressPct?.toFixed(0) || "0";
      return { label: `BACKTEST ${pct}%`, color: colors.accent, pnl: null };
    }
    if (sessionStatus === "running" && portfolio) {
      const totalPnl = portfolio.totalPnl ?? 0;
      const returnPct = portfolio.returnPct ?? 0;
      const pnlColor = totalPnl >= 0 ? colors.success : colors.error;
      return {
        label: viewMode.toUpperCase(),
        color: colors.success,
        pnl: { value: totalPnl, pct: returnPct, color: pnlColor },
      };
    }
    return { label: viewMode.toUpperCase(), color: colors.textDim, pnl: null };
  };

  const mode = getModeDisplay();

  const isBacktestRunning = viewMode === "backtest" && backtestProgress.status === "running";
  const live = isBacktestRunning ? backtestProgress.liveSnapshot : undefined;

  const cash = live?.cash ?? portfolio?.cash ?? 0;
  const equity = live?.equity ?? portfolio?.equity ?? 0;
  const initialCapital = live?.initial_capital ?? portfolio?.initialCapital ?? 10000;
  const returnPct = live?.return_pct ?? portfolio?.returnPct ?? 0;
  const pnl = isBacktestRunning
    ? (live?.total_pnl ?? 0)
    : (portfolio?.unrealizedPnl ?? 0);
  const invested = live?.invested
    ?? positions.reduce((sum, p) => sum + (p.entryPrice ?? 0) * (p.quantity ?? 0), 0);

  return (
    <box
      style={{
        flexDirection: "column",
        backgroundColor: colors.bgAlt,
        paddingLeft: 1,
        paddingRight: 1,
      }}
    >
      {/* top line: branding + mode */}
      <box style={{ flexDirection: "row", justifyContent: "space-between" }}>
        <text>
          <span fg={colors.text}>OpenMarketUI</span>
          <span fg={colors.textDim}> / </span>
          <span fg={colors.accent}>watchtower</span>
        </text>
        <text>
          <span fg={mode.color}>{mode.label}</span>
          {mode.pnl && (
            <span fg={mode.pnl.color}>
              {" "}{formatPnl(mode.pnl.value)} ({formatPct(mode.pnl.pct)})
            </span>
          )}
          <span fg={colors.textDim}> {"\u2502"} Ctrl+P</span>
        </text>
      </box>

      {/* portfolio metrics line */}
      <box style={{ flexDirection: "row", gap: 3 }}>
        <text>
          <span fg={colors.textDim}>cash </span>
          <span fg={colors.text}>${cash.toFixed(2)}</span>
        </text>
        <text>
          <span fg={colors.textDim}>invested </span>
          <span fg={colors.text}>${invested.toFixed(2)}</span>
        </text>
        <text>
          <span fg={colors.textDim}>equity </span>
          <span fg={equity >= initialCapital ? colors.success : colors.error}>
            ${equity.toFixed(2)}
          </span>
        </text>
        <text>
          <span fg={colors.textDim}>return </span>
          <span fg={returnPct >= 0 ? colors.success : colors.error}>
            {returnPct >= 0 ? "+" : ""}{returnPct.toFixed(2)}%
          </span>
        </text>
        <text>
          <span fg={colors.textDim}>P&L </span>
          <span fg={pnl >= 0 ? colors.success : colors.error}>
            {pnl >= 0 ? "+" : ""}${pnl.toFixed(2)}
          </span>
        </text>
      </box>
    </box>
  );
}
