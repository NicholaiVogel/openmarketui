import { useColors } from "../../hooks";
import type { TabName } from "../../types";

interface TabBarProps {
  active: TabName;
  onSelect: (tab: TabName) => void;
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

export function TabBar({ active }: TabBarProps) {
  const colors = useColors();
  return (
    <box
      style={{
        flexDirection: "row",
        paddingLeft: 1,
        paddingTop: 0,
        paddingBottom: 0,
      }}
    >
      {tabs.map((tab, idx) => {
        const isActive = active === tab.key;
        return (
          <text key={tab.key}>
            {idx > 0 && <span fg={colors.textDim}> │ </span>}
            <span fg={isActive ? colors.accent : colors.textDim}>
              {tab.shortcut}
            </span>
            <span fg={isActive ? colors.text : colors.textDim}>
              ·{tab.label}
            </span>
          </text>
        );
      })}
    </box>
  );
}
