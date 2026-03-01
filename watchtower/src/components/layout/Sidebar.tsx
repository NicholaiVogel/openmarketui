import { useColors } from "../../hooks";
import type { TabName, Alert } from "../../types";

interface SidebarProps {
  active: TabName;
  connected: boolean;
  alerts?: Alert[];
  onAcknowledge?: () => void;
}

const tabs: { key: TabName; label: string; shortcut: string }[] = [
  { key: "overview", label: "overview", shortcut: "1" },
  { key: "positions", label: "positions", shortcut: "2" },
  { key: "trades", label: "trades", shortcut: "3" },
  { key: "engine", label: "engine", shortcut: "4" },
  { key: "decisions", label: "decisions", shortcut: "5" },
  { key: "timeline", label: "timeline", shortcut: "6" },
  { key: "data", label: "data", shortcut: "7" },
];

export function Sidebar({ active, alerts = [] }: SidebarProps) {
  const colors = useColors();
  const unacknowledged = alerts.filter((a) => !a.acknowledged);
  const alertCount = unacknowledged.length;

  return (
    <box
      style={{
        flexDirection: "row",
        justifyContent: "center",
        paddingTop: 1,
        paddingBottom: 0,
        gap: 0,
      }}
    >
      {tabs.map((tab, idx) => {
        const isActive = active === tab.key;
        return (
          <text key={tab.key}>
            {idx > 0 && <span fg={colors.border}> {"\u2502"} </span>}
            <span fg={isActive ? colors.accent : colors.textDim}>
              {tab.shortcut}
            </span>
            <span fg={isActive ? colors.text : colors.textDim}>
              {" "}{tab.label}
            </span>
          </text>
        );
      })}

      {alertCount > 0 && (
        <text>
          <span fg={colors.border}> {"\u2502"} </span>
          <span fg={colors.warning}>!</span>
          <span fg={colors.textDim}>{alertCount} [a]</span>
        </text>
      )}
    </box>
  );
}
