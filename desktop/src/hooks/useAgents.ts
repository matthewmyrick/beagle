// The agents hook: subscribes to the daemon's live `agents-status` stream
// (with an immediate one-shot fetch so the view renders before the first
// push), and exposes start/stop/reload actions. Everything degrades to an
// offline event when beagle-agentd isn't running.

import { useCallback, useEffect, useState } from "react";

import {
  agentsStatus,
  onAgentsStatus,
  reloadAgentsConfig,
  startAgent,
  stopAgent,
} from "../api";
import type { AgentsEvent } from "../types";

function describe(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return typeof error === "string" ? error : "unknown error";
}

export interface Agents {
  /** The latest daemon status, or `null` before the first update. */
  event: AgentsEvent | null;
  /** The last action error / notice, if any. */
  notice: string | null;
  start: (id: string) => void;
  stop: (id: string) => void;
  reload: () => void;
}

export function useAgents(): Agents {
  const [event, setEvent] = useState<AgentsEvent | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    // One-shot fetch so we don't wait up to a poll interval for the first push.
    agentsStatus()
      .then((status) => {
        if (!cancelled) {
          setEvent({ connected: true, status });
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setEvent({ connected: false, error: describe(error) });
        }
      });

    onAgentsStatus((next) => {
      setEvent(next);
    })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch(() => {
        // Without the event stream the one-shot fetch above still rendered.
      });

    return () => {
      cancelled = true;
      if (unlisten !== null) {
        unlisten();
      }
    };
  }, []);

  const start = useCallback((id: string): void => {
    startAgent(id).catch((error: unknown) => {
      setNotice(describe(error));
    });
  }, []);

  const stop = useCallback((id: string): void => {
    stopAgent(id).catch((error: unknown) => {
      setNotice(describe(error));
    });
  }, []);

  const reload = useCallback((): void => {
    reloadAgentsConfig().then(
      () => {
        setNotice("config reloaded");
      },
      (error: unknown) => {
        setNotice(describe(error));
      },
    );
  }, []);

  return { event, notice, start, stop, reload };
}
