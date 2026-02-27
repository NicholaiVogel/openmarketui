import { useEffect } from "react";
import { useColors, useModeStore } from "../../hooks";

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(0)}k`;
  return n.toString();
}

function formatDateShort(dateStr: string): string {
  const date = new Date(dateStr + "T00:00:00");
  return date.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

export function DateRangePicker() {
  const colors = useColors();
  const {
    dateRangeIndex,
    dataAvailability,
    customStartDate,
    customEndDate,
    fetchDataAvailability,
  } = useModeStore();

  useEffect(() => {
    fetchDataAvailability();
  }, [fetchDataAvailability]);

  // Generate presets based on available data
  const getPresets = () => {
    if (!dataAvailability?.start_date || !dataAvailability?.end_date) {
      return [];
    }

    const start = dataAvailability.start_date;
    const end = dataAvailability.end_date;
    const presets: Array<{ name: string; start: string; end: string }> = [];

    // Full available range
    presets.push({
      name: "Full available range",
      start,
      end,
    });

    // If more than 1 day, offer subsets
    if (dataAvailability.days_count > 1) {
      const startDate = new Date(start + "T00:00:00");
      const endDate = new Date(end + "T00:00:00");
      const daysDiff = Math.ceil((endDate.getTime() - startDate.getTime()) / (1000 * 60 * 60 * 24));

      if (daysDiff >= 2) {
        const midDate = new Date(startDate);
        midDate.setDate(midDate.getDate() + Math.floor(daysDiff / 2));
        const midStr = midDate.toISOString().split("T")[0] || end;
        presets.push({
          name: "First half",
          start,
          end: midStr,
        });
        presets.push({
          name: "Second half",
          start: midStr,
          end,
        });
      }
    }

    return presets;
  };

  const presets = getPresets();
  const isCustomSelected = dateRangeIndex >= presets.length;

  if (!dataAvailability) {
    return (
      <box style={{ flexDirection: "column" }}>
        <text fg={colors.textDim}>loading available data...</text>
      </box>
    );
  }

  if (!dataAvailability.has_data) {
    return (
      <box style={{ flexDirection: "column" }}>
        <text fg={colors.warning}>no historical data available</text>
        <text fg={colors.textDim} style={{ marginTop: 1 }}>
          download data first via [d] in the modes menu
        </text>
        <box style={{ marginTop: 2 }}>
          <text fg={colors.textDim}>[h] back</text>
        </box>
      </box>
    );
  }

  return (
    <box style={{ flexDirection: "column" }}>
      <text fg={colors.accent}>available data:</text>
      <text fg={colors.text}>
        {dataAvailability.start_date} to {dataAvailability.end_date}
      </text>
      <text fg={colors.textDim}>
        {formatNumber(dataAvailability.total_trades)} trades ({dataAvailability.days_count} days)
      </text>

      <text fg={colors.textDim} style={{ marginTop: 1 }}>
        ---
      </text>

      <text fg={colors.text} style={{ marginTop: 1 }}>
        select period:
      </text>

      {presets.map((preset, idx) => {
        const isSelected = idx === dateRangeIndex;
        return (
          <box
            key={preset.name}
            style={{
              flexDirection: "row",
              backgroundColor: isSelected ? colors.bgAlt : undefined,
            }}
          >
            <text fg={isSelected ? colors.accent : colors.text}>
              {isSelected ? "> " : "  "}
              {preset.name}
            </text>
            <text fg={colors.textDim}>
              {" "}({formatDateShort(preset.start)} - {formatDateShort(preset.end)})
            </text>
          </box>
        );
      })}

      <box
        style={{
          flexDirection: "column",
          marginTop: 1,
          backgroundColor: isCustomSelected ? colors.bgAlt : undefined,
        }}
      >
        <text fg={isCustomSelected ? colors.accent : colors.text}>
          {isCustomSelected ? "> " : "  "}
          Custom range
        </text>
        {isCustomSelected && (
          <box style={{ flexDirection: "column", marginLeft: 2, marginTop: 1 }}>
            <box style={{ flexDirection: "row" }}>
              <text fg={colors.textDim}>start: </text>
              <text fg={colors.text}>{customStartDate || dataAvailability.start_date}</text>
            </box>
            <box style={{ flexDirection: "row" }}>
              <text fg={colors.textDim}>end:   </text>
              <text fg={colors.text}>{customEndDate || dataAvailability.end_date}</text>
            </box>
            <text fg={colors.textDim} style={{ marginTop: 1 }}>
              (uses full range - custom input coming soon)
            </text>
          </box>
        )}
      </box>

      <box style={{ flexDirection: "row", gap: 2, marginTop: 2 }}>
        <text fg={colors.textDim}>[enter] select</text>
        <text fg={colors.textDim}>[h] back</text>
      </box>
    </box>
  );
}
