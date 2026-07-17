// Ranking for the \ global finder, pure and testable: fuzzy-score every
// corpus line against the query, best first, capped. An empty query
// lists the corpus in natural order so the popup is useful on open.

import { score } from "./fuzzy";

/** One searchable line — mirrors CorpusLine in src-tauri/src/dto.rs. */
export interface CorpusLine {
  id: string;
  title: string;
  file: string;
  line: number;
  text: string;
}

export const MAX_RESULTS = 100;

export function rankCorpus(corpus: readonly CorpusLine[], query: string): CorpusLine[] {
  if (query === "") {
    return corpus.slice(0, MAX_RESULTS);
  }
  return corpus
    .map((entry) => ({ entry, rank: score(query, entry.text) }))
    .filter((hit): hit is { entry: CorpusLine; rank: number } => hit.rank !== null)
    .sort((a, b) => b.rank - a.rank)
    .slice(0, MAX_RESULTS)
    .map((hit) => hit.entry);
}
