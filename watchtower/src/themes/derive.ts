import type { ThemeColors, ThemeDefinition } from "./types";

export function normalizeHex(hex: string): string {
  let h = hex.trim();
  if (!h.startsWith("#")) h = `#${h}`;
  if (h.length === 4) {
    h = `#${h[1]}${h[1]}${h[2]}${h[2]}${h[3]}${h[3]}`;
  }
  return h.toUpperCase();
}

function hexToRgb(hex: string): [number, number, number] {
  const h = normalizeHex(hex);
  const r = parseInt(h.slice(1, 3), 16);
  const g = parseInt(h.slice(3, 5), 16);
  const b = parseInt(h.slice(5, 7), 16);
  return [r, g, b];
}

function rgbToHex(r: number, g: number, b: number): string {
  const clamp = (n: number) => Math.max(0, Math.min(255, Math.round(n)));
  return `#${clamp(r).toString(16).padStart(2, "0")}${clamp(g).toString(16).padStart(2, "0")}${clamp(b).toString(16).padStart(2, "0")}`.toUpperCase();
}

export function mix(hexA: string, hexB: string, t: number): string {
  const [r1, g1, b1] = hexToRgb(hexA);
  const [r2, g2, b2] = hexToRgb(hexB);
  const r = r1 + (r2 - r1) * t;
  const g = g1 + (g2 - g1) * t;
  const b = b1 + (b2 - b1) * t;
  return rgbToHex(r, g, b);
}

export function deriveThemeColors(def: ThemeDefinition): ThemeColors {
  const { colors } = def;
  return {
    bg: colors.bg,
    bgAlt: colors.bgAlt ?? mix(colors.bg, colors.text, 0.08),
    border: colors.border,
    text: colors.text,
    textDim: colors.textDim,
    accent: colors.accent,
    success: colors.success,
    warning: colors.warning,
    error: colors.error,
    blooming: colors.blooming ?? colors.success,
    dormant: colors.dormant ?? colors.textDim,
    pruned: colors.pruned ?? colors.textDim,
  };
}
