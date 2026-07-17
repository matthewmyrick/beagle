// ANSI SGR → HTML for diagrams. The diagram .txt files carry color as
// zero-width SGR escapes (red = broken, green = healthy, yellow =
// degraded); converting them to styled spans keeps the color on the web
// while preserving the exact character alignment escapes are zero-width,
// and so are the spans we emit. Mirrors cli/src/ansi.rs; malformed or
// unknown escapes degrade to plain text rather than throwing.

const TOKEN = /\u001b\[([0-9;]*)([a-zA-Z])|([^\u001b]+)|\u001b/g;

const COLORS: Readonly<Record<number, string>> = {
  0: "black",
  1: "red",
  2: "green",
  3: "yellow",
  4: "blue",
  5: "magenta",
  6: "cyan",
  7: "white",
};

interface Style {
  bold: boolean;
  dim: boolean;
  fg: string | null;
}

/** Removes every ANSI escape, leaving plain text (alignment preserved). */
export function stripAnsi(text: string): string {
  return text.replace(/\u001b\[[0-9;]*[a-zA-Z]/g, "").replace(/\u001b/g, "");
}

/** Converts ANSI SGR color/bold codes to HTML spans; other escapes are
 *  stripped. Text is HTML-escaped. */
export function ansiToHtml(text: string): string {
  const style: Style = { bold: false, dim: false, fg: null };
  let out = "";
  for (const match of text.matchAll(TOKEN)) {
    const params = match[1];
    const final = match[2];
    const chunk = match[3];
    if (final === "m") {
      applySgr(style, params ?? "");
    } else if (final === undefined && chunk !== undefined) {
      out += wrap(chunk, style);
    }
    // Non-SGR CSI (final defined, ≠ "m") and lone ESC are dropped.
  }
  return out;
}

function applySgr(style: Style, params: string): void {
  const codes = params === "" ? [0] : params.split(";").map(Number);
  for (const code of codes) {
    if (code === 0) {
      style.bold = false;
      style.dim = false;
      style.fg = null;
    } else if (code === 1) {
      style.bold = true;
    } else if (code === 2) {
      style.dim = true;
    } else if (code === 22) {
      style.bold = false;
      style.dim = false;
    } else if (code === 39) {
      style.fg = null;
    } else if (code >= 30 && code <= 37) {
      style.fg = COLORS[code - 30] ?? null;
    } else if (code >= 90 && code <= 97) {
      style.fg = COLORS[code - 90] ?? null;
    }
  }
}

function wrap(text: string, style: Style): string {
  const escaped = escapeHtml(text);
  const classes: string[] = [];
  if (style.bold) {
    classes.push("ansi-bold");
  }
  if (style.dim) {
    classes.push("ansi-dim");
  }
  if (style.fg !== null) {
    classes.push(`ansi-${style.fg}`);
  }
  return classes.length === 0
    ? escaped
    : `<span class="${classes.join(" ")}">${escaped}</span>`;
}

function escapeHtml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
