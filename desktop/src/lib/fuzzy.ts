// Case-insensitive subsequence fuzzy scoring — a faithful port of the
// CLI's fuzzy::score (cli/src/fuzzy.rs), so the desktop filter ranks
// exactly like the TUI's. Consecutive runs and word-boundary hits score
// higher; `payapi` ranks `payments-api` above scattered letters.

const BOUNDARY = new Set([" ", "-", "_", "/", ".", ":"]);

/**
 * Scores `needle` against `haystack`; higher is better. `null` when the
 * needle's characters do not all appear in order. An empty needle
 * matches everything with score 0.
 */
export function score(needle: string, haystack: string): number | null {
  if (needle === "") {
    return 0;
  }
  const n = needle.toLowerCase();
  const hay = haystack.toLowerCase();

  let total = 0;
  let ni = 0;
  let lastHit = -2;

  for (let hi = 0; hi < hay.length; hi += 1) {
    if (ni < n.length && hay[hi] === n[ni]) {
      total += 1;
      if (lastHit + 1 === hi) {
        total += 4; // consecutive run beats scattered letters
      }
      const previous = hay[hi - 1];
      if (hi === 0 || (previous !== undefined && BOUNDARY.has(previous))) {
        total += 3; // start of a word
      }
      lastHit = hi;
      ni += 1;
    }
  }

  const lengthPenalty = Math.floor(hay.length / 16);
  return ni === n.length ? total - lengthPenalty : null;
}
