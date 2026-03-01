# OpenMarketUI Design System Rules (Figma MCP Integration)

This document describes the design system structure, tokens, components, and patterns across the OpenMarketUI monorepo. Use it when translating Figma designs into code.

## Project Structure

```
openmarketui/
├── crates/           # Rust backend (pm-core, pm-store, pm-garden, pm-engine, pm-server, pm-kalshi)
├── watchtower/       # Terminal UI — React + OpenTUI (renders to terminal, NOT browser)
├── web/              # Marketing site — Astro + Tailwind v4 (renders to browser)
├── tools/            # Python scripts
└── compost/          # Standalone trading engines
```

**Two distinct UI surfaces exist. They share NO code, tokens, or components.**

---

## Surface 1: `web/` — Astro Marketing Site

### Frameworks & Libraries

- **Framework**: Astro 5.x (static output)
- **Styling**: Tailwind CSS v4 via `@tailwindcss/vite` plugin
- **React**: 19.x installed for JSX expressions in `.astro` files (no standalone `.tsx` components)
- **Build**: Astro + Vite
- **Deployment**: Cloudflare Pages (static)

### Token Definitions

No formal token file. Tokens are defined in two places:

**Typography** (`web/src/styles/global.css`):
```css
@theme {
  --font-sans: "Instrument Sans", system-ui, sans-serif;
  --font-display: "Outfit", system-ui, sans-serif;
  --font-mono: "IBM Plex Mono", monospace;
}
```

**Color palette** (hardcoded in markup, no variables):
| Role | Value | Usage |
|------|-------|-------|
| Text/primary | `#0a0a0a` | Body text, headings |
| Background | `#fff` / `#fafafa` | Page bg, section bg |
| Neutral scale | Tailwind `neutral-100` through `neutral-500` | Borders, muted text |
| TUI green | `#66800b` | Terminal mockup: profit/positive |
| TUI blue | `#205ea6` | Terminal mockup: accent/info |
| TUI red | `#af3029` | Terminal mockup: loss/negative |

**Fonts** (loaded from Google Fonts CDN in `Layout.astro`):
- **Outfit** (300–700): display headings
- **Instrument Sans** (400–700, italic): body text
- **IBM Plex Mono** (400, 500, 600): code, terminal mockups

**Spacing**: Tailwind defaults only — no custom spacing scale.

### Styling Approach

Tailwind v4 utility classes directly in Astro markup. No CSS Modules, no styled-components.

**Global CSS** (`web/src/styles/global.css`):
- `@import "tailwindcss"` (v4 style)
- `@layer base`: html/body resets, selection color
- `@layer utilities`: halftone dot pattern classes
- Custom `@keyframes` for animations

**Signature halftone texture system**:
```css
.halftone-sm   /* 0.5px dots, 4px grid */
.halftone-md   /* 0.8px dots, 6px grid */
.halftone-lg   /* 1.2px dots, 8px grid */
.halftone-fade     /* mask: top-to-bottom fade */
.halftone-fade-r   /* mask: right-to-left fade */
```

**Animation classes**:
- `.animate-fade-up` — translateY(20px)→0, 0.8s
- `.animate-fade-in` — opacity 0→1, 0.6s
- `.animate-slide-in-right` — translateX(40px)→0
- `.animate-dots-drift` — halftone background drift, 20s loop
- `.delay-{100..1200}` — stagger utilities
- `.hover-lift` — translateY(-2px) + shadow on hover

**Terminal line styling**:
- `.terminal-line::before` — 6px dot bullet
- Variants: `terminal-line-pass` (20%), `terminal-line-score` (50%), `terminal-line-signal` (100%)

### Component Library

**None**. The entire site is a single page (`web/src/pages/index.astro`, ~741 lines) with inline markup. The only abstraction is `Layout.astro` (head/meta shell).

When adding new pages or sections, follow the existing pattern: write directly in `.astro` files using Tailwind classes. Extract to Astro or React components only if reuse is needed.

### Icon System

No icon library. All icons are inline SVGs in the markup:
```html
<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
  <path d="M7 17L17 7M17 7H7M17 7v10"/>
</svg>
```

When adding icons from Figma: export as inline SVG, apply Tailwind sizing classes.

### Asset Management

- `web/public/` — favicon set (`favicon.ico`, `favicon.png`, `favicon.svg`), `logo.png`
- `web/brand/` — brand assets (not publicly served)
- No image optimization pipeline (raw `<img>` tags, no Astro `<Image>`)
- Fonts from Google Fonts CDN (not self-hosted)

### Design Language

- **Aggressively monochromatic**: black text on white, neutral grays
- **Halftone textures** as primary decorative element
- **Terminal aesthetic**: IBM Plex Mono, dark terminal mockups embedded in the light page
- **No dark mode** — light only
- **No theme system**

### Responsive Approach

Tailwind responsive prefixes (`sm:`, `md:`, `lg:`, `xl:`) used inline. Mobile-first. Breakpoints are Tailwind defaults.

---

## Surface 2: `watchtower/` — Terminal UI

### Frameworks & Libraries

- **Runtime**: Bun (runs `src/index.tsx` directly, no bundler)
- **Renderer**: OpenTUI (`@opentui/core` + `@opentui/react`) — renders React to terminal cells
- **React**: 19.x
- **State**: Zustand 5.x (3 stores: theme, garden, mode)

**This is NOT a browser UI.** There is no DOM, no CSS, no pixels. JSX renders to terminal primitives: `<box>`, `<text>`, `<span>`. Layout uses flexbox-like props. Spacing is in integer terminal cell units.

### Token Definitions

**12 semantic color roles** (`watchtower/src/themes/types.ts`):
```typescript
interface ThemeColors {
  bg: string;        // main background
  bgAlt: string;     // alternate bg (headers, selected rows)
  border: string;    // panel borders
  text: string;      // primary foreground
  textDim: string;   // muted/secondary text
  accent: string;    // interactive highlights (tabs, links)
  success: string;   // positive status
  warning: string;   // caution status
  error: string;     // negative status
  blooming: string;  // domain: active scorer
  dormant: string;   // domain: paused scorer
  pruned: string;    // domain: disabled scorer
}
```

**Derivation rules** (for themes that don't specify all 12):
- `bgAlt` = `mix(bg, text, 0.08)`
- `blooming` = `success`
- `dormant` = `textDim`
- `pruned` = `textDim`

**Static fallback** (`watchtower/src/config/colors.ts`):
```typescript
export const colors = {
  bg: "#0f1419", bgAlt: "#1a1f26", border: "#2d3640",
  text: "#c5c5c5", textDim: "#6e7681", accent: "#58a6ff",
  success: "#3fb950", warning: "#d29922", error: "#f85149",
  blooming: "#3fb950", dormant: "#8b949e", pruned: "#6e7681",
}
```

### Theme System

**40 built-in themes** in `watchtower/src/themes/registry.ts` + special `terminal` theme that reads ANSI palette at runtime.

Theme IDs (kebab-case, stable): `terminal`, `aura`, `carbon-fox`, `catppuccin-frappe`, `catppuccin-latte`, `catppuccin-macchiato`, `catppuccin-mocha`, `cobalt`, `curse`, `dracula`, `everforest-dark`, `everforest-light`, `flexoki-dark`, `flexoki-light`, `github-dark`, `github-light`, `gruvbox-dark`, `gruvbox-light`, `iu`, `kanagawa`, `lucent-orng`, `material`, `matrix`, `mercury`, `monokai`, `night-owl`, `nord`, `one-dark`, `orng`, `osaka-jade`, `pale-night`, `rose-pine`, `solarized`, `synthwave-84`, `tokyo-night`, `versailles`, `vesper`, `zenburn`

**Persistence**: `~/.config/watchtower/config.json`

### Styling Approach

**Inline styles only** via OpenTUI JSX props:
```tsx
<box style={{
  flexDirection: "row",
  justifyContent: "space-between",
  paddingLeft: 1,
  backgroundColor: colors.bgAlt,
}}>
  <text fg={colors.accent}>Label</text>
</box>
```

- No CSS files, no CSS-in-JS, no Tailwind
- Layout: flexbox-style props (`flexDirection`, `flexGrow`, `justifyContent`, `gap`, `padding*`, `margin*`)
- Text color: `fg` prop on `<text>` elements
- Background: `backgroundColor` style prop
- Borders: `border: true` + `borderColor` on `<box>` elements
- Modals: `position: "absolute"` with `top/left/right/bottom: 0`
- Spacing: integer terminal cells (1 = one character width/height)

### Component Library

**Shared primitives** (`watchtower/src/components/shared/`):
| Component | Purpose | Key Props |
|-----------|---------|-----------|
| `Panel` | Bordered box with title | `title`, `flexGrow`, `margin*` |
| `Table` | Generic typed data table | `columns: Column<T>[]`, `data: T[]`, `selectedIndex` |
| `Gauge` | Progress bar with thresholds | `value`, `max`, `warningThreshold`, `criticalThreshold` |
| `Badge` | Colored status pill | `variant: success\|warning\|error\|info\|muted`, `label` |
| `Histogram` | Horizontal bar chart | `data`, `barChar`, `colorFn`, `labelWidth` |

**Layout components** (`watchtower/src/components/layout/`):
- `Header` — branding + portfolio metrics bar
- `Sidebar` — horizontal tab navigation
- `StatusBar` — connection/theme/tab info bar
- `CommandMenu` — modal command palette (routes to ThemePicker, ModeSelector, ConfigEditor, etc.)

**Tab views** (`watchtower/src/components/tabs/`):
- `GardenOverview` — specimen table + sparkline + trade tape (main landing)
- `CurrentHarvest` — open positions
- `HarvestHistory` — fill/trade history
- `GreenhouseControls` — engine controls
- `DecisionFeed` — real-time decision log
- `TransactionTimeline` — chronological decisions + fills
- `ScorerDrilldown` — per-scorer analytics
- `DataCollector` — data fetch interface

### Icon System (Terminal)

Unicode characters only — no icon library:
| Char | Code | Usage |
|------|------|-------|
| `●` | `\u25CF` | Active/blooming, connection live |
| `○` | `\u25CB` | Dormant/paused |
| `✕` | `\u2715` | Pruned/disabled |
| `█` | `\u2588` | Bar fill, chart fill |
| `░` | `\u2591` | Bar empty |
| `▓` | `\u2593` | Histogram bars |
| `│` | `\u2502` | Nav separators |

### Key Pattern: Color Consumption

Every component uses the `useColors()` hook — **never hardcode hex values**:
```typescript
import { useColors } from "../hooks/useColors";

function MyComponent() {
  const colors = useColors();
  return <text fg={colors.accent}>hello</text>;
}
```

---

## Figma-to-Code Translation Rules

### For `web/` (browser):
1. Use Tailwind v4 utility classes — no custom CSS unless adding new animation/texture patterns
2. Map Figma colors to the existing monochrome palette (`#0a0a0a`, `#fff`, neutral scale)
3. Use `font-display` (Outfit) for headings, `font-sans` (Instrument Sans) for body, `font-mono` (IBM Plex Mono) for code
4. Export Figma icons as inline SVGs with Tailwind sizing
5. Apply halftone textures via existing utility classes where appropriate
6. No component library to map to — write inline Astro/Tailwind markup

### For `watchtower/` (terminal):
1. This surface cannot render Figma designs directly — it's terminal-only
2. If designing watchtower layouts in Figma, map to the 12 semantic color roles
3. All layout must use integer cell units and flexbox-style properties
4. Use the shared primitives (Panel, Table, Gauge, Badge, Histogram) when possible
5. Always consume colors via `useColors()` hook, never hardcode

### General:
- The two surfaces are completely independent — designs for one don't apply to the other
- The project uses the metaphor: strategies = "specimens", beds = strategy families, trades = "harvests", filters = "immune system"
- TypeScript strict mode: use `unknown` with narrowing, no `any`
