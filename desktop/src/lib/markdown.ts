// A small, dependency-free markdown parser for the subset RCA authors
// actually use — the same deliberate subset the TUI renders (see
// cli/src/markdown.rs): #/##/### headings, - bullets with [ ]/[x]
// checklists, ``` fences, > quotes, --- rules, **bold**, `inline code`.
// Everything else is plain text; malformed input degrades, never throws.

export type Inline =
  | { kind: "text"; text: string }
  | { kind: "bold"; text: string }
  | { kind: "code"; text: string };

export type Block =
  | { kind: "heading"; level: 1 | 2 | 3; text: string }
  | { kind: "rule" }
  | { kind: "quote"; text: string }
  | { kind: "code"; text: string }
  | { kind: "bullet"; indent: number; checkbox: "open" | "done" | null; text: string }
  | { kind: "paragraph"; text: string };

/** Splits one line into text/bold/code runs. Unbalanced markers stay literal. */
export function parseInline(text: string): Inline[] {
  const runs: Inline[] = [];
  let plain = "";
  let rest = text;
  const flush = (): void => {
    if (plain !== "") {
      runs.push({ kind: "text", text: plain });
      plain = "";
    }
  };
  while (rest !== "") {
    if (rest.startsWith("**")) {
      const end = rest.indexOf("**", 2);
      if (end >= 0) {
        flush();
        runs.push({ kind: "bold", text: rest.slice(2, end) });
        rest = rest.slice(end + 2);
        continue;
      }
    }
    if (rest.startsWith("`")) {
      const end = rest.indexOf("`", 1);
      if (end >= 0) {
        flush();
        runs.push({ kind: "code", text: rest.slice(1, end) });
        rest = rest.slice(end + 1);
        continue;
      }
    }
    plain += rest[0] ?? "";
    rest = rest.slice(1);
  }
  flush();
  return runs;
}

function checkbox(rest: string): { state: "open" | "done"; text: string } | null {
  const match = /^\[( |x|X)\](?: (.*))?$/.exec(rest);
  if (match === null) {
    return null;
  }
  return { state: match[1] === " " ? "open" : "done", text: match[2] ?? "" };
}

/** Parses a whole document. Adjacent prose lines join into one paragraph. */
export function parseBlocks(source: string): Block[] {
  const blocks: Block[] = [];
  let inFence = false;
  // A blank line ends any paragraph/bullet run — prose after it starts
  // fresh instead of joining backwards.
  let separated = true;
  for (const raw of source.split("\n")) {
    const trimmed = raw.trimStart();
    const indent = raw.length - trimmed.length;
    if (trimmed.startsWith("```")) {
      inFence = !inFence;
      if (inFence) {
        blocks.push({ kind: "code", text: "" });
      }
      continue;
    }
    if (inFence) {
      const last = blocks.at(-1);
      if (last?.kind === "code") {
        last.text = last.text === "" ? raw : `${last.text}\n${raw}`;
      }
      continue;
    }
    if (trimmed === "") {
      separated = true;
      continue;
    }
    const wasSeparated = separated;
    separated = false;
    const heading = /^(#{1,3}) (.*)$/.exec(trimmed);
    if (heading !== null) {
      const level = (heading[1]?.length ?? 1) as 1 | 2 | 3;
      blocks.push({ kind: "heading", level, text: heading[2] ?? "" });
      continue;
    }
    if (trimmed === "---" || trimmed === "***") {
      blocks.push({ kind: "rule" });
      continue;
    }
    if (trimmed.startsWith("> ") || trimmed === ">") {
      blocks.push({ kind: "quote", text: trimmed.slice(2) });
      continue;
    }
    if (trimmed.startsWith("- ") || trimmed.startsWith("* ")) {
      const rest = trimmed.slice(2);
      const box = checkbox(rest);
      blocks.push({
        kind: "bullet",
        indent,
        checkbox: box === null ? null : box.state,
        text: box === null ? rest : box.text,
      });
      continue;
    }
    // Prose: an indented line continues a bullet; otherwise it joins the
    // previous paragraph so hard-wrapped source reads as one flow.
    const last = blocks.at(-1);
    if (!wasSeparated && indent >= 2 && last?.kind === "bullet") {
      last.text = `${last.text} ${trimmed}`;
      continue;
    }
    if (!wasSeparated && last?.kind === "paragraph") {
      last.text = `${last.text} ${trimmed}`;
      continue;
    }
    blocks.push({ kind: "paragraph", text: trimmed });
  }
  return blocks;
}
