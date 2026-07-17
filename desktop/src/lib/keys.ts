// The keybinding table: one source of truth rendered by the help overlay
// and dispatched by useKeybindings, so the help sheet can never drift
// from what the keys actually do.

export type Action =
  | "next-incident"
  | "previous-incident"
  | "next-tab"
  | "previous-tab"
  | "jump-tab"
  | "focus-filter"
  | "toggle-archived"
  | "toggle-theme"
  | "open-finder"
  | "toggle-help";

export interface Binding {
  /** Display label for the help sheet. */
  readonly label: string;
  /** What it does, for the help sheet. */
  readonly describe: string;
  readonly action: Action;
}

export const BINDINGS: readonly Binding[] = [
  { label: "j / ↓", describe: "next incident", action: "next-incident" },
  { label: "k / ↑", describe: "previous incident", action: "previous-incident" },
  { label: "] / →", describe: "next tab", action: "next-tab" },
  { label: "[ / ←", describe: "previous tab", action: "previous-tab" },
  { label: "1–9", describe: "jump to a tab", action: "jump-tab" },
  {
    label: "/ or f",
    describe: "filter the incident list (esc clears)",
    action: "focus-filter",
  },
  { label: "a", describe: "show / hide archived incidents", action: "toggle-archived" },
  { label: "t", describe: "toggle light / dark theme", action: "toggle-theme" },
  {
    label: "\\",
    describe: "find everywhere: fuzzy across all incidents",
    action: "open-finder",
  },
  { label: "?", describe: "this help (any key closes)", action: "toggle-help" },
];

/** Maps a keyboard event's key to its action; `null` when unbound. */
export function actionForKey(key: string): { action: Action; tabIndex?: number } | null {
  switch (key) {
    case "j":
    case "ArrowDown":
      return { action: "next-incident" };
    case "k":
    case "ArrowUp":
      return { action: "previous-incident" };
    case "]":
    case "ArrowRight":
      return { action: "next-tab" };
    case "[":
    case "ArrowLeft":
      return { action: "previous-tab" };
    case "/":
    case "f":
      return { action: "focus-filter" };
    case "a":
      return { action: "toggle-archived" };
    case "t":
      return { action: "toggle-theme" };
    case "\\":
      return { action: "open-finder" };
    case "?":
      return { action: "toggle-help" };
    default: {
      if (/^[1-9]$/.test(key)) {
        return { action: "jump-tab", tabIndex: Number(key) - 1 };
      }
      return null;
    }
  }
}
