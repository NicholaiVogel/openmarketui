export type AlertSeverity = "info" | "warning" | "critical";

export interface Alert {
  id: string;
  timestamp: string;
  severity: AlertSeverity;
  title: string;
  message: string;
  acknowledged: boolean;
  source: "circuit_breaker" | "connection" | "position" | "system";
}
