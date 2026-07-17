// The \ global finder: telescope-style discovery across every incident,
// tab, and line. Type to re-rank, ↑/↓ or hover to move the single
// selection, enter or click jumps, esc closes. Matched characters
// highlight in the accent color.

import { useEffect, useMemo, useRef, useState } from "react";
import type { JSX } from "react";

import { searchCorpus } from "../api";
import { rankCorpus, toRuns } from "../lib/finder";
import type { CorpusLine, FinderHit } from "../lib/finder";
import { SECTIONS } from "../lib/sections";

interface FinderOverlayProps {
  onJump: (target: CorpusLine) => void;
  onClose: () => void;
  onError: (message: string) => void;
}

function tabTitle(file: string): string {
  return SECTIONS.find((section) => section.file === file)?.title ?? file;
}

function HitText({ hit }: { hit: FinderHit }): JSX.Element {
  return (
    <span className="finder-text">
      {toRuns(hit.entry.text, hit.positions).map((run, i) =>
        run.matched ? (
          <mark key={i} className="finder-match">
            {run.text}
          </mark>
        ) : (
          <span key={i}>{run.text}</span>
        ),
      )}
    </span>
  );
}

export function FinderOverlay({
  onJump,
  onClose,
  onError,
}: FinderOverlayProps): JSX.Element {
  const [corpus, setCorpus] = useState<CorpusLine[] | null>(null);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const selectedRef = useRef<HTMLLIElement | null>(null);
  // Rows re-render and scroll under a stationary cursor, which fires
  // synthetic hover events; selection must only follow *real* pointer
  // movement, and only keyboard moves may scroll the list — otherwise
  // hover → scroll → new row under cursor → hover… cascades.
  const lastInteraction = useRef<"keyboard" | "mouse">("keyboard");
  const lastPointer = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    inputRef.current?.focus();
    let stale = false;
    searchCorpus()
      .then((lines) => {
        if (!stale) {
          setCorpus(lines);
        }
      })
      .catch((cause: unknown) => {
        onError(String(cause));
      });
    return () => {
      stale = true;
    };
  }, [onError]);

  const hits = useMemo(() => rankCorpus(corpus ?? [], query), [corpus, query]);
  const clamped = Math.min(selected, Math.max(0, hits.length - 1));

  // Keyboard selection must stay visible as it walks past the fold.
  // Mouse-driven selection never scrolls — the pointer is already there.
  useEffect(() => {
    if (lastInteraction.current === "keyboard") {
      selectedRef.current?.scrollIntoView({ block: "nearest" });
    }
  }, [clamped]);

  const handleKey = (event: React.KeyboardEvent<HTMLInputElement>): void => {
    if (event.key === "Escape") {
      onClose();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      lastInteraction.current = "keyboard";
      setSelected(Math.min(hits.length - 1, clamped + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      lastInteraction.current = "keyboard";
      setSelected(Math.max(0, clamped - 1));
    } else if (event.key === "Enter") {
      const hit = hits[clamped];
      if (hit !== undefined) {
        onJump(hit.entry);
      }
    }
  };

  return (
    <div className="overlay-backdrop" onClick={onClose} role="presentation">
      <div
        className="finder-sheet fade-in"
        role="dialog"
        aria-label="Find everywhere"
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <input
          ref={inputRef}
          className="finder-input"
          type="search"
          placeholder="find everywhere…"
          value={query}
          onChange={(event) => {
            lastInteraction.current = "keyboard";
            setQuery(event.target.value);
            setSelected(0);
          }}
          onKeyDown={handleKey}
        />
        <ul className="finder-results">
          {corpus === null ? <li className="finder-empty">building corpus…</li> : null}
          {corpus !== null && hits.length === 0 ? (
            <li className="finder-empty">no matches</li>
          ) : null}
          {hits.map((hit, index) => (
            <li
              key={`${hit.entry.id}:${hit.entry.file}:${String(hit.entry.line)}`}
              ref={index === clamped ? selectedRef : null}
            >
              <button
                type="button"
                className={index === clamped ? "finder-row selected" : "finder-row"}
                onMouseMove={(event) => {
                  const moved =
                    lastPointer.current?.x !== event.clientX ||
                    lastPointer.current.y !== event.clientY;
                  lastPointer.current = { x: event.clientX, y: event.clientY };
                  if (moved && index !== clamped) {
                    lastInteraction.current = "mouse";
                    setSelected(index);
                  }
                }}
                onClick={() => {
                  onJump(hit.entry);
                }}
              >
                <span className="finder-context">
                  {hit.entry.title} · {tabTitle(hit.entry.file)}
                </span>
                <HitText hit={hit} />
              </button>
            </li>
          ))}
        </ul>
        <p className="help-hint">↑/↓ move · enter jump · esc close</p>
      </div>
    </div>
  );
}
