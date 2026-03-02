import type { ThemeColors } from "./types";
import { mix } from "./derive";

export interface TerminalPalette {
  defaultBackground?: string | null;
  defaultForeground?: string | null;
  palette?: (string | null)[];
}

const FALLBACK_BG = "#0f1419";
const FALLBACK_FG = "#c5c5c5";
const FALLBACK_GRAY = "#6e7681";
const FALLBACK_RED = "#f85149";
const FALLBACK_GREEN = "#3fb950";
const FALLBACK_YELLOW = "#d29922";
const FALLBACK_BLUE = "#58a6ff";

export function buildTerminalTheme(palette: TerminalPalette): ThemeColors {
  const bg = palette.defaultBackground ?? FALLBACK_BG;
  const text = palette.defaultForeground ?? FALLBACK_FG;
  const colors = palette.palette ?? [];

  const red = colors[1] ?? FALLBACK_RED;
  const green = colors[2] ?? FALLBACK_GREEN;
  const yellow = colors[3] ?? FALLBACK_YELLOW;
  const blue = colors[4] ?? FALLBACK_BLUE;
  const gray = colors[8] ?? FALLBACK_GRAY;

  const bgAlt = mix(bg, text, 0.08);
  const border = mix(bg, text, 0.18);
  const textDim = gray;

  return {
    bg,
    bgAlt,
    border,
    text,
    textDim,
    accent: blue,
    success: green,
    warning: yellow,
    error: red,
    blooming: green,
    dormant: textDim,
    pruned: textDim,
  };
}
