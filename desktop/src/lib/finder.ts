// Ranking for the \ global finder, pure and testable: fuzzy-score every
// corpus line against the query, best first, capped, with the matched
// character positions for highlighting. An empty query lists the corpus
// in natural order so the popup is useful on open.

import { indices } from "./fuzzy";

/** One searchable line — mirrors CorpusLine in src-tauri/src/dto.rs. */
export interface CorpusLine {
  id: string;
  title: string;
  file: string;
  line: number;
  text: string;
}

/** A ranked hit: the line plus which chars of its text matched. */
export interface FinderHit {
  entry: CorpusLine;
  positions: number[];
}

export const MAX_RESULTS = 100;

export function rankCorpus(corpus: readonly CorpusLine[], query: string): FinderHit[] {
  if (query === "") {
    return corpus.slice(0, MAX_RESULTS).map((entry) => ({ entry, positions: [] }));
  }
  return corpus
    .map((entry) => ({ entry, match: indices(query, entry.text) }))
    .filter(
      (
        hit,
      ): hit is { entry: CorpusLine; match: { score: number; positions: number[] } } =>
        hit.match !== null,
    )
    .sort((a, b) => b.match.score - a.match.score)
    .slice(0, MAX_RESULTS)
    .map((hit) => ({ entry: hit.entry, positions: hit.match.positions }));
}

/** Splits `text` into runs of matched/unmatched chars for rendering. */
export function toRuns(
  text: string,
  positions: readonly number[],
): { text: string; matched: boolean }[] {
  const matched = new Set(positions);
  const runs: { text: string; matched: boolean }[] = [];
  for (let i = 0; i < text.length; i += 1) {
    const isMatch = matched.has(i);
    const last = runs.at(-1);
    if (last?.matched === isMatch) {
      last.text += text[i] ?? "";
    } else {
      runs.push({ text: text[i] ?? "", matched: isMatch });
    }
  }
  return runs;
}
