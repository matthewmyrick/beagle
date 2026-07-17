// Theme selection logic, kept pure so it's testable without a DOM. The
// hook in hooks/useTheme.ts owns the side effects (localStorage, the
// data-theme attribute).

export type Theme = "dark" | "light";

export const THEME_STORAGE_KEY = "beagle-theme";

/**
 * The theme to start with: an explicit stored choice wins; anything else
 * (including junk in storage) falls back to the OS preference.
 */
export function resolveTheme(stored: string | null, prefersDark: boolean): Theme {
  if (stored === "dark" || stored === "light") {
    return stored;
  }
  return prefersDark ? "dark" : "light";
}

/** The other theme — what the toggle switches to. */
export function nextTheme(theme: Theme): Theme {
  return theme === "dark" ? "light" : "dark";
}
