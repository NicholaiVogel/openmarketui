export interface FetchProgressSnapshot {
  phase?: string | null;
  current_day: string | null;
  days_complete: number;
  days_total: number;
  trades_fetched: number;
  markets_fetched?: number;
  markets_done?: boolean;
}

export interface DownloadProgressView {
  stage: "trades" | "markets" | "starting" | "complete" | "cancelled" | "failed";
  title: string;
  description: string;
  currentLabel: string;
  currentValue: string;
  progressLabel: string;
  completed: number;
  total: number;
  percent: number;
}

const MARKET_STEP_RE = /^market\s+(\d+)\/(\d+)\s+(.+)$/;

function finiteNumber(value: number | undefined | null): number {
  return Number.isFinite(value) ? Number(value) : 0;
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

export function formatCompactNumber(value: number | undefined | null): string {
  const n = finiteNumber(value);
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${Math.round(n)}`;
}

export function truncateMiddle(value: string, maxLength: number): string {
  if (value.length <= maxLength) return value;
  if (maxLength <= 3) return value.slice(0, maxLength);

  const left = Math.ceil((maxLength - 1) / 2);
  const right = Math.floor((maxLength - 1) / 2);
  return `${value.slice(0, left)}…${value.slice(value.length - right)}`;
}

function parseMarketStep(currentDay: string | null): {
  current: number;
  total: number;
  ticker: string;
} | null {
  if (!currentDay) return null;

  const match = currentDay.match(MARKET_STEP_RE);
  if (!match) return null;

  const current = Number(match[1]);
  const total = Number(match[2]);
  const ticker = match[3] ?? "";

  if (!Number.isFinite(current) || !Number.isFinite(total) || total <= 0) {
    return null;
  }

  return { current, total, ticker };
}

export function describeDownloadProgress(
  progress: FetchProgressSnapshot
): DownloadProgressView {
  const phase = progress.phase;

  if (phase === "complete") {
    return {
      stage: "complete",
      title: "dataset ready",
      description: "Trades and market metadata are available for backtests.",
      currentLabel: "status",
      currentValue: "complete",
      progressLabel: "complete",
      completed: 1,
      total: 1,
      percent: 100,
    };
  }

  if (phase === "cancelled") {
    return {
      stage: "cancelled",
      title: "download cancelled",
      description: "The current dataset build was stopped.",
      currentLabel: "status",
      currentValue: "cancelled",
      progressLabel: "cancelled",
      completed: 0,
      total: 1,
      percent: 0,
    };
  }

  if (phase === "fetching_markets") {
    const parsed = parseMarketStep(progress.current_day);
    if (parsed) {
      const completed = clamp(parsed.current, 0, parsed.total);
      const percent = Math.round((completed / parsed.total) * 100);

      return {
        stage: "markets",
        title: "enriching market definitions",
        description:
          "Attaching titles, close times, status, and outcomes so replayed trades become a usable simulation.",
        currentLabel: "market",
        currentValue: truncateMiddle(parsed.ticker, 36),
        progressLabel: `${completed}/${parsed.total} markets`,
        completed,
        total: parsed.total,
        percent,
      };
    }

    return {
      stage: "markets",
      title: "enriching market definitions",
      description:
        "Attaching market context so the backtest can replay historical trades against real contract metadata.",
      currentLabel: "market",
      currentValue: progress.current_day || "starting...",
      progressLabel: "market metadata",
      completed: progress.markets_done ? 1 : 0,
      total: 1,
      percent: progress.markets_done ? 100 : 0,
    };
  }

  if (phase === "fetching_trades") {
    const daysTotal = Math.max(1, Math.floor(finiteNumber(progress.days_total)));
    const daysComplete = clamp(
      Math.floor(finiteNumber(progress.days_complete)),
      0,
      daysTotal
    );
    const percent = Math.round((daysComplete / daysTotal) * 100);

    return {
      stage: "trades",
      title: "collecting historical trades",
      description:
        "Downloading dated trade prints that become the price path for backtest replay.",
      currentLabel: "day",
      currentValue: progress.current_day || "starting...",
      progressLabel: `${daysComplete}/${daysTotal} days`,
      completed: daysComplete,
      total: daysTotal,
      percent,
    };
  }

  return {
    stage: "starting",
    title: "starting dataset build",
    description:
      "Preparing a historical corpus for backtesting, trade replay, and model training.",
    currentLabel: "status",
    currentValue: progress.current_day || "starting...",
    progressLabel: "starting",
    completed: 0,
    total: 1,
    percent: 0,
  };
}
