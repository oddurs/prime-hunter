import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { useWebSocket } from "@/hooks/use-websocket";

interface MockWsInstance {
  onopen: (() => void) | null;
  onclose: (() => void) | null;
  onerror: (() => void) | null;
  onmessage: ((e: { data: string }) => void) | null;
  close: ReturnType<typeof vi.fn>;
  send: ReturnType<typeof vi.fn>;
  readyState: number;
}

describe("useWebSocket", () => {
  let wsInstances: MockWsInstance[];

  beforeEach(() => {
    wsInstances = [];

    // Use a proper class so `new WebSocket(...)` works
    class MockWebSocket {
      static OPEN = 1;
      static CLOSED = 3;
      static CONNECTING = 0;
      static CLOSING = 2;

      onopen: (() => void) | null = null;
      onclose: (() => void) | null = null;
      onerror: (() => void) | null = null;
      onmessage: ((e: { data: string }) => void) | null = null;
      close = vi.fn();
      send = vi.fn();
      readyState = 0;

      constructor() {
        wsInstances.push(this);
      }
    }

    vi.stubGlobal("WebSocket", MockWebSocket);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  function latestWs(): MockWsInstance {
    return wsInstances[wsInstances.length - 1];
  }

  it("initializes as disconnected", () => {
    const { result } = renderHook(() => useWebSocket());
    expect(result.current.connected).toBe(false);
    expect(result.current.fleet).toBeNull();
    expect(result.current.searches).toEqual([]);
  });

  it("sets connected on WebSocket open", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onopen?.();
    });

    expect(result.current.connected).toBe(true);
  });

  it("sets disconnected on WebSocket close", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onopen?.();
    });
    expect(result.current.connected).toBe(true);

    act(() => {
      latestWs().onclose?.();
    });
    expect(result.current.connected).toBe(false);
  });

  it("parses update messages", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onopen?.();
    });

    const updateMsg = {
      type: "update",
      status: { active: true, checkpoint: null },
      fleet: {
        workers: [],
        total_workers: 3,
        total_cores: 24,
        total_tested: 1000,
        total_found: 5,
      },
      searches: [{ id: 1, search_type: "kbn", status: "running" }],
      deployments: [],
      notifications: [],
    };

    act(() => {
      latestWs().onmessage?.({ data: JSON.stringify(updateMsg) });
    });

    expect(result.current.status).toEqual({ active: true, checkpoint: null });
    expect(result.current.fleet?.total_workers).toBe(3);
    expect(result.current.searches).toHaveLength(1);
  });

  it("handles notification messages", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onopen?.();
    });

    const notifMsg = {
      type: "notification",
      notification: {
        id: 1,
        kind: "prime",
        title: "New prime!",
        details: ["5!+1"],
        count: 1,
        timestamp_ms: Date.now(),
      },
    };

    act(() => {
      latestWs().onmessage?.({ data: JSON.stringify(notifMsg) });
    });

    expect(result.current.notifications).toHaveLength(1);
    expect(result.current.notifications[0].title).toBe("New prime!");
  });

  it("deduplicates notifications by id", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    const notif = {
      type: "notification",
      notification: {
        id: 1,
        kind: "prime",
        title: "New prime!",
        details: [],
        count: 1,
        timestamp_ms: Date.now(),
      },
    };

    act(() => {
      latestWs().onmessage?.({ data: JSON.stringify(notif) });
    });
    act(() => {
      latestWs().onmessage?.({ data: JSON.stringify(notif) });
    });

    expect(result.current.notifications).toHaveLength(1);
  });

  it("ignores malformed messages", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onmessage?.({ data: "not-json" });
    });

    expect(result.current.status).toBeNull();
  });

  it("sendMessage sends JSON when connected", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    const ws = latestWs();
    ws.readyState = 1; // WebSocket.OPEN

    act(() => {
      ws.onopen?.();
    });

    act(() => {
      result.current.sendMessage({ type: "test" });
    });

    expect(ws.send).toHaveBeenCalledWith('{"type":"test"}');
  });

  it("sendMessage does nothing when not open", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    const ws = latestWs();
    ws.readyState = 3; // CLOSED

    act(() => {
      result.current.sendMessage({ type: "test" });
    });

    expect(ws.send).not.toHaveBeenCalled();
  });
});
