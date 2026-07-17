// The fixes: row — one chip per attached PR (glyph + #number + live
// state, click opens in the browser) and a small attach form.

import { useState } from "react";
import type { JSX } from "react";

import { addPr, openInBrowser } from "../api";
import { glyphFor, shortLabel } from "../lib/pr";
import type { Workspace } from "../types";

interface PrChipsProps {
  workspace: Workspace;
  states: Record<string, string>;
  onChanged: () => void;
  onError: (message: string) => void;
}

export function PrChips({
  workspace,
  states,
  onChanged,
  onError,
}: PrChipsProps): JSX.Element {
  return (
    <div className="pr-row">
      {workspace.prs.length > 0 ? <span className="pr-label">fixes:</span> : null}
      {workspace.prs.map((url) => {
        const state = states[url];
        return (
          <button
            key={url}
            type="button"
            className={`pr-chip ${state ?? "unknown"}`}
            title={url}
            onClick={() => {
              openInBrowser(url).catch((cause: unknown) => {
                onError(String(cause));
              });
            }}
          >
            {glyphFor(state)} {shortLabel(url)}
            {state !== undefined ? ` ${state}` : ""}
          </button>
        );
      })}
      <AttachPr id={workspace.id} onChanged={onChanged} onError={onError} />
    </div>
  );
}

function AttachPr({
  id,
  onChanged,
  onError,
}: {
  id: string;
  onChanged: () => void;
  onError: (message: string) => void;
}): JSX.Element {
  const [open, setOpen] = useState(false);
  const [url, setUrl] = useState("");

  if (!open) {
    return (
      <button
        type="button"
        className="pr-attach"
        onClick={() => {
          setOpen(true);
        }}
      >
        + attach PR
      </button>
    );
  }
  const submit = (): void => {
    const trimmed = url.trim();
    if (trimmed === "") {
      setOpen(false);
      return;
    }
    addPr(id, trimmed)
      .then(() => {
        setOpen(false);
        setUrl("");
        onChanged();
      })
      .catch((cause: unknown) => {
        onError(String(cause));
      });
  };
  return (
    <input
      className="pr-input"
      type="url"
      placeholder="https://github.com/…/pull/123"
      value={url}
      autoFocus
      onChange={(event) => {
        setUrl(event.target.value);
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          submit();
        }
        if (event.key === "Escape") {
          setOpen(false);
          setUrl("");
        }
      }}
      onBlur={submit}
    />
  );
}
