import { useColors, useModeStore } from "../../hooks";

export function PresetPicker() {
  const colors = useColors();
  const {
    configPresets,
    selectedPresetIndex,
    menuIndex,
  } = useModeStore();

  return (
    <box style={{ flexDirection: "column" }}>
      <text fg={colors.textDim} style={{ marginBottom: 1 }}>
        select config preset (j/k, enter to select)
      </text>

      {configPresets.map((preset, idx) => {
        const isSelected = idx === menuIndex;
        const isCurrent = idx === selectedPresetIndex;

        return (
          <box
            key={`${preset.name}-${idx}`}
            style={{
              flexDirection: "row",
              backgroundColor: isSelected ? colors.bgAlt : undefined,
            }}
          >
            <text
              fg={
                isSelected
                  ? colors.accent
                  : isCurrent
                    ? colors.success
                    : colors.text
              }
            >
              {isSelected ? "> " : "  "}
              {preset.name}
            </text>
            <text fg={colors.textDim}>
              {" "}
              ${preset.initialCapital.toLocaleString()}, {preset.kellyFraction} kelly
              {isCurrent ? " *" : ""}
            </text>
          </box>
        );
      })}

      <box
        style={{
          flexDirection: "row",
          backgroundColor: menuIndex === configPresets.length ? colors.bgAlt : undefined,
        }}
      >
        <text
          fg={menuIndex === configPresets.length ? colors.accent : colors.text}
        >
          {menuIndex === configPresets.length ? "> " : "  "}
          + Create New
        </text>
      </box>

      <box style={{ flexDirection: "row", gap: 2, marginTop: 1 }}>
        <text fg={colors.textDim}>[enter] select</text>
        <text fg={colors.textDim}>[e] edit</text>
        <text fg={colors.textDim}>[d] delete</text>
        <text fg={colors.textDim}>[h] back</text>
      </box>
    </box>
  );
}
