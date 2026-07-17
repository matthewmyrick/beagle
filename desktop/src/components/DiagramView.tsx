// The Diagrams tab: one diagram at a time, prev/next cycling, rendered
// unwrapped in a horizontally scrollable pre so ASCII alignment survives.

import { useEffect, useState } from "react";
import type { JSX } from "react";

import { listDiagrams, readDiagram } from "../api";
import { ansiToHtml } from "../lib/ansi";

interface DiagramViewProps {
  id: string;
  onError: (message: string) => void;
}

/** What the last completed fetch loaded, keyed by what it was for. */
interface LoadedDiagram {
  id: string;
  name: string;
  body: string | null;
}

export function DiagramView({ id, onError }: DiagramViewProps): JSX.Element {
  const [names, setNames] = useState<string[] | null>(null);
  const [index, setIndex] = useState(0);
  const [diagram, setDiagram] = useState<LoadedDiagram | null>(null);

  useEffect(() => {
    let stale = false;
    listDiagrams(id)
      .then((result) => {
        if (!stale) {
          setNames(result);
          setIndex(0);
        }
      })
      .catch((cause: unknown) => {
        onError(String(cause));
      });
    return () => {
      stale = true;
    };
  }, [id, onError]);

  const name = names?.[index] ?? null;

  useEffect(() => {
    if (name === null) {
      return undefined;
    }
    let stale = false;
    readDiagram(id, name)
      .then((body) => {
        if (!stale) {
          setDiagram({ id, name, body });
        }
      })
      .catch((cause: unknown) => {
        onError(String(cause));
      });
    return () => {
      stale = true;
    };
  }, [id, name, onError]);

  if (names === null) {
    return <div className="section-hint">loading…</div>;
  }
  if (names.length === 0 || name === null) {
    return (
      <div className="section-hint">
        No diagrams yet — ASCII files under diagrams/ appear here.
      </div>
    );
  }
  const current = diagram !== null && diagram.id === id && diagram.name === name;
  return (
    <div className="diagram-view">
      <div className="diagram-nav">
        <button
          type="button"
          disabled={index === 0}
          onClick={() => {
            setIndex((i) => Math.max(0, i - 1));
          }}
        >
          ← prev
        </button>
        <span className="diagram-name">
          {name} [{index + 1}/{names.length}]
        </span>
        <button
          type="button"
          disabled={index >= names.length - 1}
          onClick={() => {
            setIndex((i) => Math.min(names.length - 1, i + 1));
          }}
        >
          next →
        </button>
      </div>
      {current ? (
        <pre
          className="diagram-content"
          // ansiToHtml HTML-escapes the text; only our own <span> tags are emitted.
          dangerouslySetInnerHTML={{
            __html: ansiToHtml(diagram.body ?? "(diagram vanished)"),
          }}
        />
      ) : (
        <div className="section-hint">loading…</div>
      )}
    </div>
  );
}
