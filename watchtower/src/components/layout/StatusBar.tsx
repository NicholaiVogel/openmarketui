import { useColors, useThemeStore, useGardenStore } from "../../hooks";

interface StatusBarProps {
  connected: boolean;
  lastUpdate: string;
}

export function StatusBar({ connected, lastUpdate }: StatusBarProps) {
  const colors = useColors();
  const { themeName } = useThemeStore();
  const { portfolio, activeTab } = useGardenStore();

  const positionsCount = portfolio?.positionsCount ?? 0;

  return (
    <box
      style={{
        flexDirection: "row",
        justifyContent: "space-between",
        paddingLeft: 1,
        paddingRight: 1,
        backgroundColor: colors.bgAlt,
      }}
    >
      <text>
        <span fg={connected ? colors.success : colors.error}>
          {connected ? "\u25CF" : "\u25CB"}
        </span>
        <span fg={colors.textDim}> {connected ? "live" : "offline"}</span>
        <span fg={colors.border}> {"\u2502"} </span>
        <span fg={colors.textDim}>pos </span>
        <span fg={colors.text}>{positionsCount}</span>
        <span fg={colors.border}> {"\u2502"} </span>
        <span fg={colors.textDim}>{themeName}</span>
      </text>
      <text>
        <span fg={colors.textDim}>{activeTab}</span>
        <span fg={colors.border}> {"\u2502"} </span>
        <span fg={colors.textDim}>{lastUpdate || "..."}</span>
      </text>
    </box>
  );
}
