import { mkdir, readFile, writeFile, rename } from "fs/promises";
import { existsSync } from "fs";
import { join, dirname } from "path";
import { homedir } from "os";
import type { ThemeId } from "../themes/types";

export interface UserConfig {
  themeId?: ThemeId;
}

function resolveConfigDir(): string {
  const xdg = process.env.XDG_CONFIG_HOME;
  if (xdg) return join(xdg, "watchtower");
  return join(homedir(), ".config", "watchtower");
}

export function resolveConfigPath(): string {
  return join(resolveConfigDir(), "config.json");
}

export async function loadUserConfig(): Promise<UserConfig> {
  const path = resolveConfigPath();
  if (!existsSync(path)) return {};

  try {
    const raw = await readFile(path, "utf-8");
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object") {
      return { themeId: parsed.themeId };
    }
  } catch {
    // invalid json, ignore
  }

  return {};
}

export async function saveUserConfigAtomic(config: UserConfig): Promise<void> {
  const path = resolveConfigPath();
  const dir = dirname(path);
  await mkdir(dir, { recursive: true });

  const content = JSON.stringify(config, null, 2) + "\n";
  const tmpPath = `${path}.tmp.${Date.now()}`;

  await writeFile(tmpPath, content, "utf-8");
  await rename(tmpPath, path);
}
