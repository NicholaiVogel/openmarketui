import { createCliRenderer } from "@opentui/core";
import { createRoot } from "@opentui/react";
import { App } from "./app";
import { useThemeStore } from "./hooks/useThemeStore";
import { useModeStore } from "./hooks/useModeStore";

const renderer = await createCliRenderer();
await useThemeStore.getState().init(renderer);
await useModeStore.getState().init();
createRoot(renderer).render(<App />);
