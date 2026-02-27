import { useColors, useThemeStore, useModeStore } from "../../hooks";
import { ThemePicker } from "./ThemePicker";
import { ModeSelector } from "./ModeSelector";
import { ConfigEditor } from "./ConfigEditor";
import { PresetPicker } from "./PresetPicker";
import { DateRangePicker } from "./DateRangePicker";
import { DataManager } from "./DataManager";

const ROOT_MENU_ITEMS = [
  { id: "modes", label: "Modes", hint: "trading mode" },
  { id: "themes", label: "Theme", hint: "color scheme" },
  { id: "reconnect", label: "Reconnect", hint: "reconnect to server" },
  { id: "help", label: "Help", hint: "show keybindings" },
  { id: "quit", label: "Quit", hint: "exit watchtower" },
];

export function CommandMenu() {
  const colors = useColors();
  const { menuOpen, menuMode, menuIndex } = useThemeStore();
  const { menuScreen } = useModeStore();

  if (!menuOpen && menuScreen === "closed") return null;

  const renderContent = () => {
    if (menuScreen !== "closed") {
      switch (menuScreen) {
        case "mode_select":
          return <ModeSelector />;
        case "config_edit":
          return <ConfigEditor />;
        case "preset_select":
          return <PresetPicker />;
        case "date_range":
          return <DateRangePicker />;
        case "data_manager":
          return <DataManager />;
      }
    }

    if (menuMode === "root") {
      return (
        <box style={{ flexDirection: "column" }}>
          <text fg={colors.textDim} style={{ marginBottom: 1 }}>
            ctrl+p to close
          </text>
          {ROOT_MENU_ITEMS.map((item, idx) => {
            const isSelected = idx === menuIndex;
            return (
              <box
                key={item.id}
                style={{
                  flexDirection: "row",
                  backgroundColor: isSelected ? colors.bgAlt : undefined,
                }}
              >
                <text fg={isSelected ? colors.accent : colors.text}>
                  {isSelected ? "> " : "  "}
                  {item.label}
                </text>
                <text fg={colors.textDim}> - {item.hint}</text>
              </box>
            );
          })}
        </box>
      );
    }

    if (menuMode === "themes") {
      return <ThemePicker />;
    }

    return null;
  };

  const title =
    menuScreen === "mode_select"
      ? " trading mode "
      : menuScreen === "config_edit"
        ? " edit config "
        : menuScreen === "preset_select"
          ? " select preset "
          : menuScreen === "date_range"
            ? " date range "
            : menuScreen === "data_manager"
              ? " historical data "
              : " command menu ";

  return (
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
      <box
        style={{
          border: true,
          borderColor: colors.border,
          backgroundColor: colors.bg,
          padding: 1,
          width: 60,
          flexDirection: "column",
        }}
        title={title}
      >
        {renderContent()}
      </box>
    </box>
  );
}
