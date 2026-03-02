import { useThemeStore } from "./useThemeStore";

export function useColors() {
  return useThemeStore((s) => s.colors);
}
