import { create } from "zustand";
import type { ThemeId, ThemeColors } from "../themes/types";
import type { CliRenderer } from "@opentui/core";
import { deriveThemeColors } from "../themes/derive";
import { buildTerminalTheme, type TerminalPalette } from "../themes/terminal";
import { getBuiltinTheme, getThemeName, listThemeIds } from "../themes/registry";
import { loadUserConfig, saveUserConfigAtomic } from "../config/persist";
import { colors as fallbackColors } from "../config/colors";

export type MenuMode = "root" | "themes" | "modes";

const PALETTE_POLL_INTERVAL = 2000; // check every 2 seconds

let paletteWatcherId: ReturnType<typeof setInterval> | null = null;

function palettesEqual(a: TerminalPalette | null, b: TerminalPalette | null): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  if (a.defaultBackground !== b.defaultBackground) return false;
  if (a.defaultForeground !== b.defaultForeground) return false;
  if (!a.palette && !b.palette) return true;
  if (!a.palette || !b.palette) return false;
  if (a.palette.length !== b.palette.length) return false;
  for (let i = 0; i < a.palette.length; i++) {
    if (a.palette[i] !== b.palette[i]) return false;
  }
  return true;
}

interface ThemeStore {
  themeId: ThemeId;
  themeName: string;
  colors: ThemeColors;
  terminalPalette: TerminalPalette | null;
  renderer: CliRenderer | null;

  menuOpen: boolean;
  menuMode: MenuMode;
  menuIndex: number;

  init: (renderer: CliRenderer) => Promise<void>;
  setTheme: (id: ThemeId, options?: { persist?: boolean }) => Promise<void>;
  cycleTheme: (delta: number) => Promise<void>;

  openMenu: () => void;
  closeMenu: () => void;
  toggleMenu: () => void;
  setMenuMode: (mode: MenuMode) => void;
  setMenuIndex: (index: number) => void;
  moveMenuIndex: (delta: number) => void;
  handleMenuKey: (key: { name?: string; sequence?: string; ctrl?: boolean }) => { handled: boolean; action?: "reconnect" | "help" | "modes" };
}

function resolveColors(
  id: ThemeId,
  terminalPalette: TerminalPalette | null
): ThemeColors {
  if (id === "terminal" && terminalPalette) {
    return buildTerminalTheme(terminalPalette);
  }
  const builtin = getBuiltinTheme(id);
  if (builtin) {
    return deriveThemeColors(builtin);
  }
  return fallbackColors as ThemeColors;
}

function startPaletteWatcher(store: typeof useThemeStore) {
  if (paletteWatcherId) return;

  paletteWatcherId = setInterval(async () => {
    const { themeId, renderer, terminalPalette } = store.getState();
    if (themeId !== "terminal" || !renderer) return;

    try {
      const newPalette = await renderer.getPalette({ size: 16 });
      if (!palettesEqual(terminalPalette, newPalette)) {
        const colors = buildTerminalTheme(newPalette);
        try {
          renderer.setBackgroundColor(colors.bg);
        } catch {
          // ignore
        }
        store.setState({ terminalPalette: newPalette, colors });
      }
    } catch {
      // palette detection failed, ignore
    }
  }, PALETTE_POLL_INTERVAL);
}

function stopPaletteWatcher() {
  if (paletteWatcherId) {
    clearInterval(paletteWatcherId);
    paletteWatcherId = null;
  }
}

export const useThemeStore = create<ThemeStore>((set, get) => ({
  themeId: "terminal",
  themeName: "Terminal",
  colors: fallbackColors as ThemeColors,
  terminalPalette: null,
  renderer: null,

  menuOpen: false,
  menuMode: "root",
  menuIndex: 0,

  init: async (renderer) => {
    let terminalPalette: TerminalPalette | null = null;
    try {
      terminalPalette = await renderer.getPalette({ size: 16 });
    } catch {
      // palette detection not supported
    }

    const config = await loadUserConfig();
    const envTheme = process.env.WATCHTOWER_THEME as ThemeId | undefined;
    const themeIds = listThemeIds();

    let themeId: ThemeId = "terminal";
    if (envTheme && themeIds.includes(envTheme)) {
      themeId = envTheme;
    } else if (config.themeId && themeIds.includes(config.themeId)) {
      themeId = config.themeId;
    }

    const colors = resolveColors(themeId, terminalPalette);
    const themeName = getThemeName(themeId);

    try {
      renderer.setBackgroundColor(colors.bg);
    } catch {
      // some renderers might not support this
    }

    set({ themeId, themeName, colors, terminalPalette, renderer });

    // start watching if using terminal theme
    if (themeId === "terminal") {
      startPaletteWatcher(useThemeStore);
    }
  },

  setTheme: async (id, options = {}) => {
    const { renderer } = get();
    let { terminalPalette } = get();

    // re-fetch terminal palette when switching to terminal theme
    if (id === "terminal" && renderer) {
      try {
        terminalPalette = await renderer.getPalette({ size: 16 });
        set({ terminalPalette });
      } catch {
        // palette detection not supported
      }
      startPaletteWatcher(useThemeStore);
    } else {
      stopPaletteWatcher();
    }

    const colors = resolveColors(id, terminalPalette);
    const themeName = getThemeName(id);

    if (renderer) {
      try {
        renderer.setBackgroundColor(colors.bg);
      } catch {
        // ignore
      }
    }

    set({ themeId: id, themeName, colors });

    if (options.persist) {
      await saveUserConfigAtomic({ themeId: id });
    }
  },

  cycleTheme: async (delta) => {
    const { themeId } = get();
    const ids = listThemeIds();
    const idx = ids.indexOf(themeId);
    const newIdx = (idx + delta + ids.length) % ids.length;
    const newThemeId = ids[newIdx];
    if (newThemeId) {
      await get().setTheme(newThemeId, { persist: true });
    }
  },

  openMenu: () => set({ menuOpen: true, menuMode: "root", menuIndex: 0 }),
  closeMenu: () => set({ menuOpen: false, menuMode: "root", menuIndex: 0 }),
  toggleMenu: () => {
    const { menuOpen } = get();
    if (menuOpen) {
      get().closeMenu();
    } else {
      get().openMenu();
    }
  },

  setMenuMode: (mode) => set({ menuMode: mode, menuIndex: 0 }),
  setMenuIndex: (index) => set({ menuIndex: index }),

  moveMenuIndex: (delta) => {
    const { menuMode, menuIndex } = get();
    let maxIndex = 0;

    if (menuMode === "root") {
      maxIndex = 4; // Modes, Theme, Reconnect, Help, Quit
    } else if (menuMode === "themes") {
      maxIndex = listThemeIds().length - 1;
    }

    const newIndex = Math.max(0, Math.min(maxIndex, menuIndex + delta));
    set({ menuIndex: newIndex });
  },

  handleMenuKey: (key) => {
    const { menuOpen, menuMode, menuIndex, themeId } = get();
    const keyName = key.name || key.sequence;

    if (!menuOpen) return { handled: false };

    if (keyName === "escape" || (keyName === "p" && key.ctrl)) {
      get().closeMenu();
      return { handled: true };
    }

    if (keyName === "j" || keyName === "down") {
      get().moveMenuIndex(1);
      return { handled: true };
    }

    if (keyName === "k" || keyName === "up") {
      get().moveMenuIndex(-1);
      return { handled: true };
    }

    if (keyName === "h" || keyName === "left") {
      if (menuMode === "themes") {
        get().setMenuMode("root");
      }
      return { handled: true };
    }

    if (keyName === "enter" || keyName === "return" || keyName === "l" || keyName === "right") {
      if (menuMode === "root") {
        const items = ["modes", "themes", "reconnect", "help", "quit"];
        const selected = items[menuIndex];
        if (selected === "modes") {
          get().closeMenu();
          return { handled: true, action: "modes" as const };
        } else if (selected === "themes") {
          const themeIds = listThemeIds();
          const currentIdx = themeIds.indexOf(themeId);
          set({ menuMode: "themes", menuIndex: Math.max(0, currentIdx) });
        } else if (selected === "reconnect") {
          get().closeMenu();
          return { handled: true, action: "reconnect" as const };
        } else if (selected === "help") {
          get().closeMenu();
          return { handled: true, action: "help" as const };
        } else if (selected === "quit") {
          get().closeMenu();
          process.exit(0);
        }
        return { handled: true };
      } else if (menuMode === "themes") {
        const themeIds = listThemeIds();
        const selectedId = themeIds[menuIndex];
        if (selectedId) {
          get().setTheme(selectedId, { persist: true });
          get().closeMenu();
        }
        return { handled: true };
      }
    }

    return { handled: true };
  },
}));
