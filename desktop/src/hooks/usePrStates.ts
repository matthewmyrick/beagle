// Live PR states for the selected workspace: fetched when the PR set
// changes and re-polled every five minutes while the app is open. An
// empty result (no gh, no auth) leaves PRs as plain links.

import { useEffect, useState } from "react";

import { prStates } from "../api";

const POLL_MS = 5 * 60 * 1000;

export function usePrStates(urls: readonly string[]): Record<string, string> {
  const [states, setStates] = useState<Record<string, string>>({});
  // Effect dependency by value, not array identity.
  const key = urls.join("\n");

  useEffect(() => {
    if (key === "") {
      return undefined;
    }
    let stale = false;
    const fetchStates = (): void => {
      prStates(key.split("\n"))
        .then((result) => {
          if (!stale) {
            setStates((current) => ({ ...current, ...result }));
          }
        })
        .catch(() => {
          // Missing gh or network trouble degrades to plain links.
        });
    };
    fetchStates();
    const timer = window.setInterval(fetchStates, POLL_MS);
    return () => {
      stale = true;
      window.clearInterval(timer);
    };
  }, [key]);

  return states;
}
