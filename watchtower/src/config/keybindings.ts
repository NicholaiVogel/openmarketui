export const keybindings = {
  // tab switching
  tabs: {
    overview: "1",
    positions: "2",
    trades: "3",
    engine: "4",
    decisions: "5",
    timeline: "6",
    data: "7",
  },

  // global
  quit: ["q", "escape"],
  reconnect: "r",
  help: "?",
  acknowledgeAlerts: "a",
  commandMenu: { key: "p", ctrl: true },

  // navigation
  down: ["j", "down"],
  up: ["k", "up"],
  first: "g",
  last: "G",
  enter: ["enter", "return", "l"],
  back: "h",

  // controls
  pause: "p",
  toggle: "t",
  weightUp: "+",
  weightDown: "-",
} as const;

export function matchesKey(
  key: string,
  binding: string | readonly string[]
): boolean {
  if (Array.isArray(binding)) {
    return binding.includes(key);
  }
  return key === binding;
}
