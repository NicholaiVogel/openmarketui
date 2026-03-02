import { useColors } from "../../hooks";
import { Panel } from "../shared/Panel";
import { Gauge } from "../shared/Gauge";
import type {
  Bed,
  EngineStatus,
  CircuitBreakerStatus,
  MarketDecision,
} from "../../types";

interface GreenhouseControlsProps {
  beds: Bed[];
  engineStatus: EngineStatus | null;
  circuitBreaker: CircuitBreakerStatus | null;
  selectedIndex: number;
  decisions: MarketDecision[];
}

function computeLatencyStats(decisions: MarketDecision[]): {
  avg: number;
  p50: number;
  p95: number;
  p99: number;
  recent: number[];
} | null {
  const latencies = decisions
    .filter((d) => d.latencyMs !== undefined && d.latencyMs > 0)
    .map((d) => d.latencyMs!);

  if (latencies.length === 0) {
    return null;
  }

  const sorted = [...latencies].sort((a, b) => a - b);
  const avg = latencies.reduce((a, b) => a + b, 0) / latencies.length;
  const p50 = sorted[Math.floor(sorted.length * 0.5)] ?? 0;
  const p95 = sorted[Math.floor(sorted.length * 0.95)] ?? sorted[sorted.length - 1] ?? 0;
  const p99 = sorted[Math.floor(sorted.length * 0.99)] ?? sorted[sorted.length - 1] ?? 0;

  return {
    avg,
    p50,
    p95,
    p99,
    recent: latencies.slice(0, 10),
  };
}

export function GreenhouseControls({
  beds,
  engineStatus,
  circuitBreaker,
  selectedIndex,
  decisions,
}: GreenhouseControlsProps) {
  const colors = useColors();
  const allSpecimens = beds.flatMap((bed) => bed.specimens);
  const latencyStats = computeLatencyStats(decisions);

  return (
    <box style={{ flexDirection: "column", flexGrow: 1 }}>
      {/* engine status */}
      <Panel title="engine status" marginBottom={1}>
        <box style={{ flexDirection: "column", gap: 1 }}>
          <text>
            <span fg={colors.textDim}>state: </span>
            <span
              fg={
                engineStatus?.state === "Running"
                  ? colors.success
                  : engineStatus?.state.startsWith("Paused")
                    ? colors.warning
                    : colors.textDim
              }
            >
              {engineStatus?.state || "unknown"}
            </span>
          </text>
          <text>
            <span fg={colors.textDim}>uptime: </span>
            <span fg={colors.text}>
              {engineStatus ? formatUptime(engineStatus.uptimeSecs) : "-"}
            </span>
          </text>
          <text>
            <span fg={colors.textDim}>ticks: </span>
            <span fg={colors.text}>
              {engineStatus?.ticksCompleted ?? 0}
            </span>
          </text>
          <text>
            <span fg={colors.textDim}>last tick: </span>
            <span fg={colors.text}>
              {engineStatus?.lastTick
                ? new Date(engineStatus.lastTick).toLocaleTimeString()
                : "-"}
            </span>
          </text>
        </box>
      </Panel>

      {/* circuit breaker */}
      <Panel title="circuit breaker" marginBottom={1}>
        <box style={{ flexDirection: "column", gap: 1 }}>
          <Gauge
            label="drawdown"
            value={circuitBreaker?.drawdownPct ?? 0}
            max={10}
            warningThreshold={5}
            criticalThreshold={8}
            suffix="%"
          />
          <Gauge
            label="daily loss"
            value={circuitBreaker?.dailyLossPct ?? 0}
            max={5}
            warningThreshold={3}
            criticalThreshold={4}
            suffix="%"
          />
          <Gauge
            label="positions"
            value={circuitBreaker?.openPositions ?? 0}
            max={100}
            warningThreshold={70}
            criticalThreshold={90}
          />
          <Gauge
            label="fills/hour"
            value={circuitBreaker?.fillsLastHour ?? 0}
            max={50}
            warningThreshold={35}
            criticalThreshold={45}
          />
        </box>
      </Panel>

      {/* execution latency */}
      <Panel title="execution latency" marginBottom={1}>
        {latencyStats ? (
          <box style={{ flexDirection: "column", gap: 1 }}>
            <box style={{ flexDirection: "row", gap: 3 }}>
              <text>
                <span fg={colors.textDim}>avg: </span>
                <span fg={colors.text}>{latencyStats.avg.toFixed(0)}ms</span>
              </text>
              <text>
                <span fg={colors.textDim}>p50: </span>
                <span fg={colors.text}>{latencyStats.p50}ms</span>
              </text>
              <text>
                <span fg={colors.textDim}>p95: </span>
                <span fg={latencyStats.p95 > 100 ? colors.warning : colors.text}>
                  {latencyStats.p95}ms
                </span>
              </text>
              <text>
                <span fg={colors.textDim}>p99: </span>
                <span fg={latencyStats.p99 > 200 ? colors.error : colors.text}>
                  {latencyStats.p99}ms
                </span>
              </text>
            </box>
            <text>
              <span fg={colors.textDim}>recent: </span>
              <span fg={colors.accent}>
                {latencyStats.recent.map((l) => `[${l}]`).join(" ")}
              </span>
            </text>
          </box>
        ) : (
          <text fg={colors.textDim}>no latency data yet</text>
        )}
      </Panel>

      {/* specimen controls */}
      <Panel title="scorers [t] toggle, [+/-] weight" flexGrow={1}>
        {allSpecimens.length === 0 ? (
          <text fg={colors.textDim}>no scorers loaded</text>
        ) : (
          <box style={{ flexDirection: "column", gap: 0 }}>
            <box style={{ flexDirection: "row" }}>
              <text style={{ width: 20 }} fg={colors.textDim}>
                name
              </text>
              <text style={{ width: 15 }} fg={colors.textDim}>
                group
              </text>
              <text style={{ width: 12 }} fg={colors.textDim}>
                status
              </text>
              <text fg={colors.textDim}>weight</text>
            </box>
            {allSpecimens.map((specimen, idx) => {
              const weight = specimen.weight ?? 0;
              return (
                <box
                  key={specimen.name ?? idx}
                  style={{
                    flexDirection: "row",
                    backgroundColor:
                      selectedIndex === idx ? colors.bgAlt : undefined,
                  }}
                >
                  <text style={{ width: 20 }} fg={colors.accent}>
                    {selectedIndex === idx ? "> " : "  "}
                    {(specimen.name ?? "unknown").slice(0, 16)}
                  </text>
                  <text style={{ width: 15 }} fg={colors.textDim}>
                    {specimen.bed ?? "-"}
                  </text>
                  <text
                    style={{ width: 12 }}
                    fg={
                      specimen.status === "blooming"
                        ? colors.blooming
                        : specimen.status === "dormant"
                          ? colors.dormant
                          : colors.pruned
                    }
                  >
                    {specimen.status ?? "unknown"}
                  </text>
                  <text fg={colors.text}>{weight.toFixed(2)}</text>
                </box>
              );
            })}
          </box>
        )}
      </Panel>

      {/* keybindings */}
      <Panel title="controls" marginTop={1}>
        <box style={{ flexDirection: "row", gap: 3 }}>
          <text>
            <span fg={colors.accent}>[p]</span>
            <span fg={colors.textDim}> pause/resume</span>
          </text>
          <text>
            <span fg={colors.accent}>[j/k]</span>
            <span fg={colors.textDim}> navigate</span>
          </text>
          <text>
            <span fg={colors.accent}>[t]</span>
            <span fg={colors.textDim}> toggle</span>
          </text>
          <text>
            <span fg={colors.accent}>[+/-]</span>
            <span fg={colors.textDim}> weight</span>
          </text>
        </box>
      </Panel>
    </box>
  );
}

function formatUptime(secs: number): string {
  const hours = Math.floor(secs / 3600);
  const mins = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (hours > 0) {
    return `${hours}h ${mins}m ${s}s`;
  }
  if (mins > 0) {
    return `${mins}m ${s}s`;
  }
  return `${s}s`;
}
