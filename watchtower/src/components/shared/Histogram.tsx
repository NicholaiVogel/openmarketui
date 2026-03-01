import { useColors } from "../../hooks";

interface HistogramBar {
  label: string;
  value: number;
}

interface HistogramProps {
  data: HistogramBar[];
  maxWidth?: number;
  barChar?: string;
  labelWidth?: number;
  showValues?: boolean;
  colorFn?: (value: number, max: number) => string;
}

export function Histogram({
  data,
  maxWidth = 40,
  barChar = "▓",
  labelWidth = 8,
  showValues = true,
  colorFn,
}: HistogramProps) {
  const colors = useColors();
  const maxValue = Math.max(...data.map((d) => d.value), 1);

  return (
    <box style={{ flexDirection: "column", gap: 0 }}>
      {data.map((item, idx) => {
        const barLength = Math.round((item.value / maxValue) * maxWidth);
        const bar = barChar.repeat(barLength);
        const barColor = colorFn
          ? colorFn(item.value, maxValue)
          : item.value === maxValue
            ? colors.accent
            : colors.success;

        return (
          <box key={idx} style={{ flexDirection: "row" }}>
            <text style={{ width: labelWidth }} fg={colors.textDim}>
              {item.label}
            </text>
            <text fg={barColor}>{bar}</text>
            {showValues && (
              <text fg={colors.textDim} style={{ marginLeft: 1 }}>
                {item.value}
              </text>
            )}
          </box>
        );
      })}
    </box>
  );
}
