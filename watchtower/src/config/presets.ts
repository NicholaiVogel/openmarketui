import { mkdir, readFile, writeFile, rename } from "fs/promises";
import { existsSync } from "fs";
import { join, dirname } from "path";
import { homedir } from "os";
import type { TradingConfig, FeeConfig } from "../types/mode";

function resolveConfigDir(): string {
  const xdg = process.env.XDG_CONFIG_HOME;
  if (xdg) return join(xdg, "watchtower");
  return join(homedir(), ".config", "watchtower");
}

export function resolvePresetsPath(): string {
  return join(resolveConfigDir(), "presets.json");
}

const DEFAULT_FEES: FeeConfig = {
  takerRate: 0.07,
  makerRate: 0.0175,
  maxPerContract: 0.02,
  assumeTaker: true,
  minEdgeAfterFees: 0.02,
};

export const DEFAULT_PRESETS: TradingConfig[] = [
  {
    name: "Default",
    initialCapital: 10000,
    maxPositions: 100,
    kellyFraction: 0.25,
    maxPositionPct: 0.10,
    takeProfitPct: 0.50,
    stopLossPct: 0.99,
    maxHoldHours: 48,
    minTimeToCloseHours: 2,
    maxTimeToCloseHours: 504,
    cashReservePct: 0.20,
    maxEntriesPerTick: 5,
    fees: { ...DEFAULT_FEES },
  },
  {
    name: "Conservative",
    initialCapital: 5000,
    maxPositions: 50,
    kellyFraction: 0.15,
    maxPositionPct: 0.05,
    takeProfitPct: 0.30,
    stopLossPct: 0.15,
    maxHoldHours: 24,
    minTimeToCloseHours: 4,
    maxTimeToCloseHours: 168,
    cashReservePct: 0.30,
    maxEntriesPerTick: 3,
    fees: { ...DEFAULT_FEES, minEdgeAfterFees: 0.03 },
  },
  {
    name: "Aggressive",
    initialCapital: 25000,
    maxPositions: 200,
    kellyFraction: 0.40,
    maxPositionPct: 0.20,
    takeProfitPct: 0.25,
    stopLossPct: 0.50,
    maxHoldHours: 72,
    minTimeToCloseHours: 1,
    maxTimeToCloseHours: 720,
    cashReservePct: 0.10,
    maxEntriesPerTick: 10,
    fees: { ...DEFAULT_FEES, minEdgeAfterFees: 0.015 },
  },
  {
    name: "7D High Conviction",
    initialCapital: 10000,
    maxPositions: 3,
    kellyFraction: 0.40,
    maxPositionPct: 0.95,
    takeProfitPct: 0.90,
    stopLossPct: 0.99,
    maxHoldHours: 96,
    minTimeToCloseHours: 0,
    maxTimeToCloseHours: 720,
    cashReservePct: 0.05,
    maxEntriesPerTick: 3,
    fees: { ...DEFAULT_FEES },
    backtestIntervalHours: 1,
  },
];

const HIGH_CONVICTION_PRESET_NAME = "7D High Conviction";

function ensureHighConvictionPreset(presets: TradingConfig[]): TradingConfig[] {
  if (presets.some((p) => p.name === HIGH_CONVICTION_PRESET_NAME)) {
    return presets;
  }
  const builtIn = DEFAULT_PRESETS.find((p) => p.name === HIGH_CONVICTION_PRESET_NAME);
  if (!builtIn) return presets;
  return [...presets, { ...builtIn, fees: { ...DEFAULT_FEES, ...builtIn.fees } }];
}

export async function loadPresets(): Promise<TradingConfig[]> {
  const path = resolvePresetsPath();
  if (!existsSync(path)) return [...DEFAULT_PRESETS];

  try {
    const raw = await readFile(path, "utf-8");
    const parsed = JSON.parse(raw);
    if (Array.isArray(parsed) && parsed.length > 0) {
      const defaultPreset = DEFAULT_PRESETS[0];
      if (!defaultPreset) return [...DEFAULT_PRESETS];
      const merged = parsed.map((p: Partial<TradingConfig>): TradingConfig => ({
        name: p.name ?? defaultPreset.name,
        initialCapital: p.initialCapital ?? defaultPreset.initialCapital,
        maxPositions: p.maxPositions ?? defaultPreset.maxPositions,
        kellyFraction: p.kellyFraction ?? defaultPreset.kellyFraction,
        maxPositionPct: p.maxPositionPct ?? defaultPreset.maxPositionPct,
        takeProfitPct: p.takeProfitPct ?? defaultPreset.takeProfitPct,
        stopLossPct: p.stopLossPct ?? defaultPreset.stopLossPct,
        maxHoldHours: p.maxHoldHours ?? defaultPreset.maxHoldHours,
        minTimeToCloseHours: p.minTimeToCloseHours ?? defaultPreset.minTimeToCloseHours,
        maxTimeToCloseHours: p.maxTimeToCloseHours ?? defaultPreset.maxTimeToCloseHours,
        cashReservePct: p.cashReservePct ?? defaultPreset.cashReservePct,
        maxEntriesPerTick: p.maxEntriesPerTick ?? defaultPreset.maxEntriesPerTick,
        fees: { ...DEFAULT_FEES, ...p.fees },
        backtestStart: p.backtestStart,
        backtestEnd: p.backtestEnd,
        backtestIntervalHours: p.backtestIntervalHours,
      }));
      return ensureHighConvictionPreset(merged);
    }
  } catch {
    // invalid json, return defaults
  }

  return ensureHighConvictionPreset([...DEFAULT_PRESETS]);
}

export async function savePresets(presets: TradingConfig[]): Promise<void> {
  const path = resolvePresetsPath();
  const dir = dirname(path);
  await mkdir(dir, { recursive: true });

  const content = JSON.stringify(presets, null, 2) + "\n";
  const tmpPath = `${path}.tmp.${Date.now()}`;

  await writeFile(tmpPath, content, "utf-8");
  await rename(tmpPath, path);
}
