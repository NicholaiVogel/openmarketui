import { useEffect } from "react";
import { useGardenStore, useWebSocket, useKeyboardNav, useColors, useModeStore } from "./hooks";
import {
  Header,
  Sidebar,
  StatusBar,
  CommandMenu,
  BacktestProgress,
  BacktestResultsPanel,
} from "./components/layout";
import {
  GardenOverview,
  CurrentHarvest,
  HarvestHistory,
  GreenhouseControls,
  DecisionFeed,
  TransactionTimeline,
  ScorerDrilldown,
  DataCollector,
} from "./components/tabs";

// demo data for offline mode
const DEMO_BEDS = [
  {
    name: "momentum",
    specimens: [
      { name: "momentum", bed: "momentum", status: "blooming" as const, weight: 0.15 },
      { name: "mtf_momentum", bed: "momentum", status: "blooming" as const, weight: 0.10 },
      { name: "time_decay", bed: "momentum", status: "dormant" as const, weight: 0.10 },
    ],
  },
  {
    name: "mean_reversion",
    specimens: [
      { name: "mean_reversion", bed: "mean_reversion", status: "blooming" as const, weight: 0.15 },
      { name: "bollinger", bed: "mean_reversion", status: "blooming" as const, weight: 0.10 },
    ],
  },
  {
    name: "volume",
    specimens: [
      { name: "volume", bed: "volume", status: "blooming" as const, weight: 0.10 },
      { name: "order_flow", bed: "volume", status: "dormant" as const, weight: 0.10 },
    ],
  },
  {
    name: "ensemble",
    specimens: [
      { name: "category_weighted", bed: "ensemble", status: "blooming" as const, weight: 0.20 },
    ],
  },
];

export function App() {
  const colors = useColors();
  const {
    connected,
    lastUpdate,
    activeTab,
    beds,
    positions,
    recentFills,
    equityCurve,
    portfolio,
    engineStatus,
    circuitBreaker,
    decisions,
    alerts,
    selectedIndex,
    selectedSpecimen,
    selectedMarket,
    acknowledgeAllAlerts,
  } = useGardenStore();

  const { reconnect } = useWebSocket();
  useKeyboardNav({ onReconnect: reconnect });

  const { backtestProgress, backtestResult } = useModeStore();

  // populate demo data when disconnected
  useEffect(() => {
    if (!connected && beds.length === 0) {
      useGardenStore.setState({
        beds: DEMO_BEDS,
        portfolio: {
          cash: 10000,
          equity: 10000,
          initialCapital: 10000,
          positionsValue: 0,
          returnPct: 0,
          totalReturnPct: 0,
          drawdownPct: 0,
          positionsCount: 0,
          positionCount: 0,
          realizedPnl: 0,
          unrealizedPnl: 0,
          totalPnl: 0,
        },
        engineStatus: {
          state: "Running",
          uptimeSecs: 0,
          ticksCompleted: 0,
        },
        circuitBreaker: {
          status: "OK",
          drawdownPct: 0,
          dailyLossPct: 0,
          openPositions: 0,
          fillsLastHour: 0,
        },
      });
    }
  }, [connected, beds.length]);

  const displayBeds = beds.length > 0 ? beds : DEMO_BEDS;

  return (
    <box
      style={{
        flexDirection: "column",
        flexGrow: 1,
        backgroundColor: colors.bg,
      }}
    >
      <Header />
      <Sidebar active={activeTab} connected={connected} alerts={alerts} onAcknowledge={acknowledgeAllAlerts} />

      {/* content area */}
      <box style={{ flexDirection: "column", flexGrow: 1, padding: 1 }}>
          {activeTab === "overview" && (
            <GardenOverview
              beds={displayBeds}
              portfolio={portfolio}
              engineStatus={engineStatus}
              recentFills={recentFills}
              equityCurve={equityCurve}
            />
          )}
          {activeTab === "positions" && (
            <CurrentHarvest
              positions={positions}
              selectedIndex={selectedIndex}
            />
          )}
          {activeTab === "trades" && (
            <HarvestHistory fills={recentFills} selectedIndex={selectedIndex} />
          )}
          {activeTab === "engine" && (
            <GreenhouseControls
              beds={displayBeds}
              engineStatus={engineStatus}
              circuitBreaker={circuitBreaker}
              selectedIndex={selectedIndex}
              decisions={decisions}
            />
          )}
          {activeTab === "decisions" && (
            <DecisionFeed
              decisions={decisions}
              selectedIndex={selectedIndex}
            />
          )}
          {activeTab === "timeline" && (
            <TransactionTimeline
              decisions={decisions}
              recentFills={recentFills}
              selectedIndex={selectedIndex}
            />
          )}
          {activeTab === "drilldown" && (
            <ScorerDrilldown
              beds={displayBeds}
              positions={positions}
              selectedSpecimen={selectedSpecimen}
              selectedMarket={selectedMarket}
            />
          )}
          {activeTab === "data" && (
            <DataCollector selectedIndex={selectedIndex} />
          )}
      </box>

      <StatusBar connected={connected} lastUpdate={lastUpdate} />
      <CommandMenu />

      {activeTab !== "overview" && (backtestProgress.status === "running" || backtestProgress.status === "complete" || backtestProgress.status === "failed") && (
        <box
          style={{
            position: "absolute",
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            justifyContent: "center",
            alignItems: "center",
          }}
        >
          {backtestProgress.status === "running" ? (
            <BacktestProgress />
          ) : backtestProgress.status === "failed" ? (
            <box
              style={{
                border: true,
                borderColor: colors.error,
                backgroundColor: colors.bg,
                padding: 1,
                flexDirection: "column",
                width: 50,
              }}
              title=" backtest failed "
            >
              <text fg={colors.error}>{backtestProgress.error || "unknown error"}</text>
              <text fg={colors.textDim} style={{ marginTop: 1 }}>
                [esc] dismiss
              </text>
            </box>
          ) : backtestResult ? (
            <box
              style={{
                border: true,
                borderColor: colors.border,
                backgroundColor: colors.bg,
                padding: 1,
                flexDirection: "column",
                width: 50,
              }}
              title=" backtest complete "
            >
              <BacktestResultsPanel />
              <text fg={colors.textDim} style={{ marginTop: 1 }}>
                [esc] dismiss
              </text>
            </box>
          ) : null}
        </box>
      )}
    </box>
  );
}
