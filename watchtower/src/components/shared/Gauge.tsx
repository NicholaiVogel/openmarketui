import { useColors } from "../../hooks";

interface GaugeProps {
  label: string;
  value: number;
  max: number;
  warningThreshold?: number;
  criticalThreshold?: number;
  suffix?: string;
  width?: number;
}

export function Gauge({
  label,
  value: rawValue,
  max,
  warningThreshold = max * 0.7,
  criticalThreshold = max * 0.9,
  suffix = "",
  width = 20,
}: GaugeProps) {
  const colors = useColors();
  const value = rawValue ?? 0;
  const pct = Math.min(1, value / max);
  const filled = Math.round(pct * width);

  const barColor =
    value >= criticalThreshold
      ? colors.error
      : value >= warningThreshold
        ? colors.warning
        : colors.success;

  const bar = "|".repeat(filled) + "-".repeat(width - filled);

  return (
    <box style={{ flexDirection: "row", gap: 1 }}>
      <text fg={colors.textDim} style={{ width: 15 }}>
        {label}:
      </text>
      <text>
        <span fg={barColor}>[{bar}]</span>
      </text>
      <text fg={colors.text}>
        {value.toFixed(1)}
        {suffix}
      </text>
    </box>
  );
}
