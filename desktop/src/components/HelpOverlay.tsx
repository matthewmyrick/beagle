// The ? help sheet: every binding from the single source of truth in
// lib/keys.ts. Any key or click closes it.

import type { JSX } from "react";

import { BINDINGS } from "../lib/keys";

interface HelpOverlayProps {
  onClose: () => void;
}

export function HelpOverlay({ onClose }: HelpOverlayProps): JSX.Element {
  return (
    <div className="overlay-backdrop" onClick={onClose} role="presentation">
      <div className="help-sheet fade-in" role="dialog" aria-label="Keyboard shortcuts">
        <h2>keys</h2>
        <dl>
          {BINDINGS.map((binding) => (
            <div key={binding.label} className="help-row">
              <dt>
                <kbd>{binding.label}</kbd>
              </dt>
              <dd>{binding.describe}</dd>
            </div>
          ))}
        </dl>
        <p className="help-hint">any key closes</p>
      </div>
    </div>
  );
}
