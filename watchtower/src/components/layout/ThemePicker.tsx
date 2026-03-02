import { useColors, useThemeStore } from "../../hooks";
import { listThemeIds, getThemeName } from "../../themes/registry";

export function ThemePicker() {
  const colors = useColors();
  const { themeId: currentThemeId, menuIndex } = useThemeStore();
  const themeIds = listThemeIds();

  // show a window of themes around the selected index
  const windowSize = 15;
  const halfWindow = Math.floor(windowSize / 2);
  let startIdx = Math.max(0, menuIndex - halfWindow);
  const endIdx = Math.min(themeIds.length, startIdx + windowSize);
  if (endIdx - startIdx < windowSize) {
    startIdx = Math.max(0, endIdx - windowSize);
  }
  const visibleThemes = themeIds.slice(startIdx, endIdx);

  return (
    <box style={{ flexDirection: "column" }}>
      <text fg={colors.textDim}>
        select theme (j/k, enter to apply, h to go back)
      </text>
      <text fg={colors.textDim} style={{ marginBottom: 1 }}>
        [{menuIndex + 1}/{themeIds.length}]
      </text>
      {visibleThemes.map((id, idx) => {
        const realIdx = startIdx + idx;
        const isSelected = realIdx === menuIndex;
        const isCurrent = id === currentThemeId;
        return (
          <box
            key={id}
            style={{
              backgroundColor: isSelected ? colors.bgAlt : undefined,
            }}
          >
            <text fg={isSelected ? colors.accent : isCurrent ? colors.success : colors.text}>
              {isSelected ? "> " : "  "}
              {getThemeName(id)}
              {isCurrent ? " *" : ""}
            </text>
          </box>
        );
      })}
    </box>
  );
}
