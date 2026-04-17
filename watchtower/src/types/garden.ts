export type SpecimenStatus = "blooming" | "dormant" | "pruned";

export interface Specimen {
  name: string;
  bed: string;
  status: SpecimenStatus;
  weight: number;
  hitRate?: number;
  avgContribution?: number;
}

export interface Bed {
  name: string;
  specimens: Specimen[];
}

export interface Position {
  ticker: string;
  title: string;
  category: string;
  side: "Yes" | "No";
  quantity: number;
  entryPrice: number;
  avgEntryPrice: number;
  currentPrice?: number;
  costBasis: number;
  marketValue: number;
  entryTime: string;
  closeTime?: string;
  unrealizedPnl: number;
  pnlPct: number;
  unrealizedPnlPct: number;
  hoursHeld: number;
}

export interface Fill {
  ticker: string;
  side: "Yes" | "No";
  quantity: number;
  price: number;
  timestamp: string;
  fee?: number;
  pnl?: number;
  exitReason?: string;
}

export interface EquityPoint {
  timestamp: string;
  equity: number;
  cash: number;
  positionsValue: number;
  drawdownPct: number;
}
