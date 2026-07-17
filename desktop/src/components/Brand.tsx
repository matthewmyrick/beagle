// The top-right brand corner: the BEAGLE ASCII banner (the exact art the
// TUI renders — see cli/src/banner.rs) and the theme toggle.

import type { JSX } from "react";

import type { Theme } from "../lib/theme";

const BANNER = ` ___  ___    _     ___  _     ___
| _ )| __|  /_\\   / __|| |   | __|
| _ \\| _|  / _ \\ | (_ || |__ | _|
|___/|___|/_/ \\_\\ \\___||____||___|`;

interface BrandProps {
  theme: Theme;
  onToggleTheme: () => void;
}

export function Brand({ theme, onToggleTheme }: BrandProps): JSX.Element {
  return (
    <div className="brand">
      <pre className="banner-art" aria-hidden="true">
        {BANNER}
      </pre>
      <button
        type="button"
        className="theme-toggle"
        onClick={onToggleTheme}
        aria-label={theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
        title={theme === "dark" ? "Light mode" : "Dark mode"}
      >
        {theme === "dark" ? "☀" : "☾"}
      </button>
    </div>
  );
}
