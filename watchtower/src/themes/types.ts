export type ThemeId =
  | "terminal"
  | "pale-night"
  | "rose-pine"
  | "solarized"
  | "synthwave-84"
  | "tokyo-night"
  | "versailles"
  | "vesper"
  | "zenburn"
  | "osaka-jade"
  | "orng"
  | "one-dark"
  | "nord"
  | "night-owl"
  | "monokai"
  | "mercury"
  | "matrix"
  | "material"
  | "lucent-orng"
  | "kanagawa"
  | "gruvbox-dark"
  | "gruvbox-light"
  | "github-dark"
  | "github-light"
  | "flexoki-dark"
  | "flexoki-light"
  | "everforest-dark"
  | "everforest-light"
  | "dracula"
  | "curse"
  | "cobalt"
  | "catppuccin-latte"
  | "catppuccin-frappe"
  | "catppuccin-macchiato"
  | "catppuccin-mocha"
  | "carbon-fox"
  | "iu"
  | "aura";

export type ThemeSource = "terminal" | "builtin";

export interface ThemeColors {
  bg: string;
  bgAlt: string;
  border: string;
  text: string;
  textDim: string;
  accent: string;
  success: string;
  warning: string;
  error: string;
  blooming: string;
  dormant: string;
  pruned: string;
}

export interface ThemeDefinition {
  id: ThemeId;
  name: string;
  source: ThemeSource;
  colors: Omit<ThemeColors, "bgAlt" | "blooming" | "dormant" | "pruned"> &
    Partial<Pick<ThemeColors, "bgAlt" | "blooming" | "dormant" | "pruned">>;
}
