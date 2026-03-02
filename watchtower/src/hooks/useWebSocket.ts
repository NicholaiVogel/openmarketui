import { useEffect, useRef, useCallback } from "react";
import { useGardenStore } from "./useGardenStore";
import { normalizeServerMessage } from "../wire";

const WS_URL = process.env.PM_SERVER_URL || "ws://localhost:3030/ws";
const RECONNECT_DELAY = 5000;
const PING_INTERVAL = 30000;

export function useWebSocket() {
  const wsRef = useRef<WebSocket | null>(null);
  const pingIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(
    null
  );

  const { setConnected, setWs, handleMessage, addAlert } = useGardenStore();

  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      return;
    }

    try {
      const socket = new WebSocket(WS_URL);
      wsRef.current = socket;

      socket.onopen = () => {
        setConnected(true);
        setWs(socket);

        // start ping interval
        pingIntervalRef.current = setInterval(() => {
          if (socket.readyState === WebSocket.OPEN) {
            socket.send(JSON.stringify({ type: "Ping" }));
          }
        }, PING_INTERVAL);

        addAlert({
          severity: "info",
          title: "Connected",
          message: `Connected to ${WS_URL}`,
          source: "connection",
        });
      };

      socket.onclose = () => {
        setConnected(false);
        setWs(null);

        if (pingIntervalRef.current) {
          clearInterval(pingIntervalRef.current);
          pingIntervalRef.current = null;
        }

        // schedule reconnect
        reconnectTimeoutRef.current = setTimeout(connect, RECONNECT_DELAY);
      };

      socket.onerror = () => {
        addAlert({
          severity: "warning",
          title: "Connection Error",
          message: "WebSocket connection failed",
          source: "connection",
        });
      };

      socket.onmessage = (event) => {
        try {
          const raw = JSON.parse(event.data);
          const data = normalizeServerMessage(raw);
          handleMessage(data);
        } catch (err) {
          console.error("[ws] message normalization failed:", err, event.data?.slice?.(0, 200));
          addAlert({
            severity: "warning",
            title: "Message Parse Error",
            message: String(err),
            source: "connection",
          });
        }
      };
    } catch {
      reconnectTimeoutRef.current = setTimeout(connect, RECONNECT_DELAY);
    }
  }, [setConnected, setWs, handleMessage, addAlert]);

  const disconnect = useCallback(() => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }

    if (pingIntervalRef.current) {
      clearInterval(pingIntervalRef.current);
      pingIntervalRef.current = null;
    }

    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
  }, []);

  const reconnect = useCallback(() => {
    disconnect();
    connect();
  }, [connect, disconnect]);

  useEffect(() => {
    connect();
    return () => disconnect();
  }, [connect, disconnect]);

  return { connect, disconnect, reconnect };
}
