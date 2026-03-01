# Watchtower Themes + Command Menu (Spec)

This document is the point-of-reference spec for adding a theme system to the
watchtower terminal UI ("the observation deck").

Goals
---

1) Provide multiple curated themes.
2) Default theme MUST be the terminal's own ANSI palette ("Terminal").
3) Users can change theme at runtime via an in-app command menu.
4) The selected theme MUST persist across runs (local install UX).
5) Theme switching should update the UI immediately without restarting.
6) Implementation should be self-contained within `watchtower/`.

Non-goals
---

- No remote sync.
- No per-widget theming.
- No plugin system.
- No dependency on external config frameworks.

Existing State (Baseline)
---

- Colors are currently hardcoded as a single palette in
  `watchtower/src/config/colors.ts` and imported directly by components.
- The renderer is created in `watchtower/src/index.tsx` via
  `createCliRenderer()`.
- Keyboard handling lives in `watchtower/src/hooks/useKeyboardNav.ts`.

High-Level Design
---

### Semantic theme roles

The UI should not consume raw 16/256-color palettes directly. Instead, it
should consume semantic roles.

Required roles (minimum viable):

- `bg`: main background
- `bgAlt`: alternate background (selected row, header bars)
- `border`: panel borders
- `text`: primary foreground text
- `textDim`: secondary/muted text
- `accent`: interactive highlight (tabs, links)
- `success`, `warning`, `error`: status colors

Garden-specific aliases (to avoid scattering logic):

- `blooming`
- `dormant`
- `pruned`

These may be direct colors or aliases (e.g. `blooming = success`). The key
requirement is consistent roles across themes.

### Theme sources

There are two theme sources:

1) `terminal` (dynamic): derived at runtime from the user's terminal palette.
2) `builtin` (static): named themes defined in code as hex colors.

The user selects by Theme ID. The UI always renders from the currently active
resolved `ThemeColors`.

Theme Catalog
---

Theme IDs MUST be stable, kebab-case, and used for persistence.

Required themes:

- `terminal` (default; dynamic)
- `pale-night`
- `rose-pine`
- `solarized`
- `synthwave-84`
- `tokyo-night`
- `versailles`
- `vesper`
- `zenburn`
- `osaka-jade`
- `orng`
- `one-dark`
- `nord`
- `night-owl`
- `monokai`
- `mercury`
- `matrix`
- `material`
- `lucent-orng`
- `kanagawa`
- `gruvbox-dark`
- `gruvbox-light`
- `github-dark`
- `github-light`
- `flexoki-dark`
- `flexoki-light`
- `everforest-dark`
- `everforest-light`
- `dracula`
- `curse`
- `cobalt`
- `catppuccin-latte`
- `catppuccin-frappe`
- `catppuccin-macchiato`
- `catppuccin-mocha`
- `carbon-fox`
- `iu`
- `aura`

Notes
---

- If any theme does not naturally define all roles, derive missing roles using
  deterministic transforms (see "Derivation" section).

Files / Modules to Add
---

### Theme types

Create:

- `watchtower/src/themes/types.ts`

Recommended contents:

- `export type ThemeId = ...` string union of all IDs above
- `export type ThemeSource = "terminal" | "builtin"`
- `export type ThemeColors = { bg: string; bgAlt: string; border: string; text: string; textDim: string; accent: string; success: string; warning: string; error: string; blooming: string; dormant: string; pruned: string }`
- `export type ThemeDefinition = { id: ThemeId; name: string; source: ThemeSource; colors: Omit<ThemeColors, "bgAlt" | "blooming" | "dormant" | "pruned"> & Partial<Pick<ThemeColors, "bgAlt" | "blooming" | "dormant" | "pruned">> }`

Rationale: allow optional `bgAlt` and garden aliases to be derived.

### Theme registry

Create:

- `watchtower/src/themes/registry.ts`

Responsibilities:

- Export `BUILTIN_THEMES: readonly ThemeDefinition[]`
- Export helpers:
  - `listThemeIds(): ThemeId[]` (ordered for display)
  - `getThemeName(id: ThemeId): string`
  - `getBuiltinTheme(id: ThemeId): ThemeDefinition | undefined`

### Terminal theme builder

Create:

- `watchtower/src/themes/terminal.ts`

Responsibilities:

- `buildTerminalTheme(palette: TerminalColors): ThemeColors`
- Handle missing/null colors robustly.
- Derive `bgAlt` deterministically.

Persistence (Theme Choice Across Runs)
---

The selected theme MUST be persisted locally.

### Storage location (XDG)

Use JSON stored in:

- If `$XDG_CONFIG_HOME` is set: `$XDG_CONFIG_HOME/watchtower/config.json`
- Else: `~/.config/watchtower/config.json`

### Storage format

Minimal JSON:

```json
{ "themeId": "tokyo-night" }
```

### Atomic writes

To avoid corruption:

1) Ensure parent directory exists.
2) Write to a temp file in the same directory.
3) Rename temp file to `config.json`.

### Precedence rules

On startup, compute the initial theme ID by:

1) `WATCHTOWER_THEME` env var, if set and valid Theme ID.
   - This is a session override.
   - Do not auto-write it to config unless user explicitly changes theme in UI.
2) Persisted config file themeId, if present and valid.
3) `terminal`.

Config Module
---

Create:

- `watchtower/src/config/persist.ts`

Responsibilities:

- `resolveConfigPath(): string`
- `loadUserConfig(): { themeId?: string }` (tolerant of missing/invalid JSON)
- `saveUserConfigAtomic(config: { themeId: ThemeId }): Promise<void>`

Implementation notes (Bun/Node):

- Prefer `fs/promises`.
- Use `process.env.XDG_CONFIG_HOME` and `process.env.HOME`.

Theme Store (Reactive Colors)
---

Create:

- `watchtower/src/hooks/useThemeStore.ts`

State:

- `themeId: ThemeId`
- `colors: ThemeColors`
- `themeName: string`
- `menuOpen: boolean`
- `menuMode: "root" | "themes"` (or separate component state)

Actions:

- `init(options: { renderer: CliRenderer }): Promise<void>`
  - Loads config + env overrides.
  - Detects terminal palette and constructs terminal theme.
  - Resolves the initial theme and sets `colors`.
  - Sets renderer background color (see below).
- `setTheme(themeId: ThemeId, options?: { persist?: boolean }): Promise<void>`
  - Resolves theme colors.
  - Updates store.
  - Updates renderer background (and cursor if desired).
  - If `persist`, writes config.
- `cycleTheme(delta: number): Promise<void>`
  - Uses registry order to find next/prev.
- `openMenu()` / `closeMenu()` / `toggleMenu()`

Hook:

- `watchtower/src/hooks/useColors.ts` returning `useThemeStore((s) => s.colors)`

Renderer Integration
---

`@opentui/core` supports palette detection:

- `await renderer.getPalette({ size: 16 })` returns `TerminalColors`.

In `watchtower/src/index.tsx`:

1) Create renderer.
2) Initialize theme store with renderer.
3) Render `<App />`.

The theme store should call:

- `renderer.setBackgroundColor(colors.bg)`

This ensures the global background matches the theme. UI components will still
use `backgroundColor` styles for panels/rows.

Terminal Theme Mapping (Role Mapping)
---

The `terminal` theme is derived from `TerminalColors`:

- Use `defaultBackground` as `bg` if available, else fallback.
- Use `defaultForeground` as `text` if available, else fallback.
- Use the 16-color palette for role mapping.

Recommended mapping (by typical ANSI expectations):

- `success`  = palette[2]  (green)
- `warning`  = palette[3]  (yellow)
- `error`    = palette[1]  (red)
- `accent`   = palette[4]  (blue)
- `border`   = palette[8]  (bright black / gray) or derive from bg
- `textDim`  = palette[8]  (gray) or derive from text

If any palette entry is `null`, fall back to derived values.

Derivation (for missing roles)
---

To keep behavior deterministic and avoid dependencies, implement small helpers
in `watchtower/src/themes/derive.ts`:

- `normalizeHex(hex: string): string` (ensure `#RRGGBB`)
- `mix(hexA, hexB, t)` -> hex (linear blend in sRGB is acceptable here)

Suggested derivations:

- `bgAlt`: `mix(bg, text, 0.08)` (slightly toward text)
- `border`: `mix(bg, text, 0.18)`
- `textDim`: `mix(text, bg, 0.45)`

Garden aliases:

- `blooming = success`
- `dormant = textDim`
- `pruned = textDim`

Command Menu UX
---

The UI MUST provide a command menu ("context menu") accessible from anywhere.

### Hotkey

- Open/close: `Ctrl+P`.

Implementation detail: `KeyEvent` includes modifier flags.
Detect the hotkey as:

- `key.ctrl && key.name === "p"`

### Menu behavior

- When menu is open, it captures input and prevents normal navigation.
- `Esc` closes the menu.
- `j/k` and arrow keys move selection.
- `Enter` selects.

### Menu items

Minimum root menu:

- `Theme` -> opens Theme Picker
- `Reconnect` (calls existing reconnect handler)
- `Help` (toggles existing help state)
- `Quit`

### Theme picker

- Shows ordered list of themes from the registry.
- Highlights the current theme.
- Selecting a theme applies immediately and persists.

UI Components to Add
---

Add to layout components:

- `watchtower/src/components/layout/CommandMenu.tsx`
  - Renders overlay panel.
  - Uses theme colors.
  - Reads `menuOpen/menuMode` from theme store.
- `watchtower/src/components/layout/ThemePicker.tsx` (optional split)

Integrate into `watchtower/src/app.tsx`:

- Render `<CommandMenu />` near the top-level so it overlays all tabs.

Refactor Steps (Replacing Static `colors`)
---

1) Keep `watchtower/src/config/colors.ts` as a fallback palette only.
2) Add `useColors()` hook.
3) Update all components that import `colors` to instead do:

```ts
import { useColors } from "../hooks/useColors";
const colors = useColors();
```

Files known to import `colors` today (not exhaustive):

- `watchtower/src/app.tsx`
- `watchtower/src/components/layout/Header.tsx`
- `watchtower/src/components/layout/TabBar.tsx`
- `watchtower/src/components/layout/StatusBar.tsx`
- `watchtower/src/components/layout/AlertBanner.tsx`
- `watchtower/src/components/shared/Panel.tsx`
- `watchtower/src/components/shared/Table.tsx`
- `watchtower/src/components/shared/Badge.tsx`
- Tabs under `watchtower/src/components/tabs/*.tsx`

Keyboard Routing
---

Update `watchtower/src/hooks/useKeyboardNav.ts`:

- Use `KeyEvent` modifier fields (`ctrl/meta/shift`).
- Add a check for `Ctrl+P` to toggle the command menu.
- If the menu is open, route key events to menu navigation first and `return`
  so normal UI actions do not fire.

Recommended approach:

- Store exposes `handleMenuKey(key: KeyEvent): boolean`.
- `useKeyboardNav` calls it at the top; if it returns true, stop.

Status Bar Theme Display
---

Update `watchtower/src/components/layout/StatusBar.tsx`:

- Show current theme name, e.g. `theme: tokyo-night`.
- When `terminal` is active, show `theme: terminal`.

Testing / Verification
---

Manual verification steps:

1) Start watchtower: `cd watchtower && bun dev`.
2) Confirm default theme is terminal-derived (matches terminal bg/fg feel).
3) Press `Ctrl+P` and open the menu.
4) Select a theme (e.g. `tokyo-night`). Confirm UI updates immediately.
5) Quit and relaunch. Confirm theme persists.
6) Set `WATCHTOWER_THEME=terminal` and relaunch. Confirm it overrides config
   for that session.
7) Remove/rename `~/.config/watchtower/config.json` and relaunch. Confirm
   clean fallback behavior.

Failure/edge cases to handle:

- Palette detection fails (no OSC support): fallback to built-in palette.
- Terminal returns partial/null palette entries: derive missing roles.
- Config JSON is invalid: ignore and continue.

Implementation Checklist (Do This In Order)
---

1) Add theme types + registry + derivation helpers.
2) Add persistence module.
3) Add terminal theme builder.
4) Implement theme store with `init(renderer)` and `setTheme(..., { persist })`.
5) Wire `init(renderer)` into `watchtower/src/index.tsx` before rendering.
6) Add `useColors()` and refactor components away from static `colors` import.
7) Add command menu + theme picker overlay.
8) Update keyboard nav to toggle menu on `Ctrl+P` and route keys.
9) Update status bar to display current theme.
10) Manual verification steps.
