import { useColors } from "../../hooks";
import { Panel } from "../shared/Panel";
import { Histogram } from "../shared/Histogram";
import type { Fill } from "../../types";

interface TradingRhythmProps {
  recentFills: Fill[];
}

interface RhythmBucket {
  label: string;
  value: number;
  hours: [number, number];
}

function computeRhythm(fills: Fill[]): {
  buckets: RhythmBucket[];
  peakRange: string | null;
  peakPct: number | null;
  total: number;
} {
  const ranges: Array<{ label: string; hours: [number, number] }> = [
    { label: "00-03", hours: [0, 3] },
    { label: "03-06", hours: [3, 6] },
    { label: "06-09", hours: [6, 9] },
    { label: "09-12", hours: [9, 12] },
    { label: "12-15", hours: [12, 15] },
    { label: "15-18", hours: [15, 18] },
    { label: "18-21", hours: [18, 21] },
    { label: "21-24", hours: [21, 24] },
  ];

  const counts = new Array(8).fill(0);

  for (const fill of fills) {
    try {
      const date = new Date(fill.timestamp);
      const hour = date.getHours();
      const bucketIdx = Math.floor(hour / 3);
      if (bucketIdx >= 0 && bucketIdx < 8) {
        counts[bucketIdx]++;
      }
    } catch {
      // ignore invalid timestamps
    }
  }

  const total = counts.reduce((a, b) => a + b, 0);
  const buckets: RhythmBucket[] = ranges.map((r, i) => ({
    label: r.label,
    value: counts[i],
    hours: r.hours,
  }));

  let peakRange: string | null = null;
  let peakPct: number | null = null;

  if (total > 0) {
    const maxIdx = counts.indexOf(Math.max(...counts));
    const range = ranges[maxIdx];
    if (range && counts[maxIdx] > 0) {
      peakRange = range.label;
      peakPct = (counts[maxIdx] / total) * 100;
    }
  }

  return { buckets, peakRange, peakPct, total };
}

export function TradingRhythm({ recentFills }: TradingRhythmProps) {
  const colors = useColors();
  const rhythm = computeRhythm(recentFills);

  const histogramData = rhythm.buckets.map((b) => ({
    label: b.label,
    value: b.value,
  }));

  return (
    <box style={{ flexDirection: "column", flexGrow: 1 }}>
      <Panel title="trading rhythm" flexGrow={1}>
        {rhythm.total === 0 ? (
          <box style={{ flexDirection: "column", gap: 1 }}>
            <text fg={colors.textDim}>no fills recorded yet...</text>
            <text fg={colors.textDim}>
              trading patterns will appear here as fills accumulate
            </text>
          </box>
        ) : (
          <box style={{ flexDirection: "column", gap: 1 }}>
            <text fg={colors.textDim}>hour fills</text>
            <Histogram
              data={histogramData}
              maxWidth={40}
              labelWidth={8}
              showValues={true}
            />

            <box style={{ marginTop: 1 }}>
              {rhythm.peakRange && rhythm.peakPct && (
                <text>
                  <span fg={colors.textDim}>peak: </span>
                  <span fg={colors.accent}>{rhythm.peakRange}</span>
                  <span fg={colors.textDim}>
                    {" "}
                    ({rhythm.peakPct.toFixed(1)}% of trades)
                  </span>
                </text>
              )}
            </box>

            <text fg={colors.textDim}>
              total fills: {rhythm.total} (from recent history)
            </text>
          </box>
        )}
      </Panel>

      <Panel title="day of week" marginTop={1}>
        <DayOfWeekRhythm fills={recentFills} />
      </Panel>

      <Panel title="legend" marginTop={1}>
        <box style={{ flexDirection: "row", gap: 3 }}>
          <text>
            <span fg={colors.success}>▓</span>
            <span fg={colors.textDim}> fills in bucket</span>
          </text>
          <text>
            <span fg={colors.accent}>▓</span>
            <span fg={colors.textDim}> peak bucket</span>
          </text>
        </box>
      </Panel>
    </box>
  );
}

function DayOfWeekRhythm({ fills }: { fills: Fill[] }) {
  const colors = useColors();
  const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const counts = new Array(7).fill(0);

  for (const fill of fills) {
    try {
      const date = new Date(fill.timestamp);
      const day = date.getDay();
      counts[day]++;
    } catch {
      // ignore invalid timestamps
    }
  }

  const total = counts.reduce((a, b) => a + b, 0);

  if (total === 0) {
    return <text fg={colors.textDim}>no data</text>;
  }

  const data = days.map((label, i) => ({ label, value: counts[i] }));

  return (
    <Histogram
      data={data}
      maxWidth={30}
      labelWidth={5}
      showValues={true}
    />
  );
}
