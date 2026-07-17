// The \ global finder: telescope-style discovery across every incident,
// tab, and line. Type to re-rank, ↑/↓ to move, enter jumps, esc closes.

import { useEffect, useMemo, useRef, useState } from "react";
import type { JSX } from "react";

import { searchCorpus } from "../api";
import { rankCorpus } from "../lib/finder";
import type { CorpusLine } from "../lib/finder";
import { SECTIONS } from "../lib/sections";

interface FinderOverlayProps {
  onJump: (target: CorpusLine) => void;
  onClose: () => void;
  onError: (message: string) => void;
}

function tabTitle(file: string): string {
  return SECTIONS.find((section) => section.file === file)?.title ?? file;
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

  const handleKey = (event: React.KeyboardEvent<HTMLInputElement>): void => {
    if (event.key === "Escape") {
      onClose();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelected((current) => Math.min(hits.length - 1, current + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelected((current) => Math.max(0, current - 1));
    } else if (event.key === "Enter") {
      const hit = hits[clamped];
      if (hit !== undefined) {
        onJump(hit);
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
            <li key={`${hit.id}:${hit.file}:${String(hit.line)}`}>
              <button
                type="button"
                className={index === clamped ? "finder-row selected" : "finder-row"}
                onClick={() => {
                  onJump(hit);
                }}
              >
                <span className="finder-context">
                  {hit.title} · {tabTitle(hit.file)}
                </span>
                <span className="finder-text">{hit.text}</span>
              </button>
            </li>
          ))}
        </ul>
        <p className="help-hint">↑/↓ move · enter jump · esc close</p>
      </div>
    </div>
  );
}
