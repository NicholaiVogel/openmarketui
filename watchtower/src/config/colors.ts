export const colors = {
  bg: "#0f1419",
  bgAlt: "#1a1f26",
  border: "#2d3640",
  text: "#c5c5c5",
  textDim: "#6e7681",
  accent: "#58a6ff",
  success: "#3fb950",
  warning: "#d29922",
  error: "#f85149",
  blooming: "#3fb950",
  dormant: "#8b949e",
  pruned: "#6e7681",
} as const;

export type ColorKey = keyof typeof colors;
