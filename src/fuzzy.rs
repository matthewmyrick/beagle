//! Case-insensitive subsequence fuzzy matching for the incident filter.
//!
//! Small on purpose: the corpus is a handful of workspace titles, not a code
//! search index. Scoring rewards consecutive runs and word-boundary hits so
//! `payapi` ranks `payments-api` above `p...a...y` scattered across a title.

/// Scores `needle` against `haystack`; higher is better. Returns `None`
/// when the needle's characters do not all appear in order.
///
/// An empty needle matches everything with score 0, so an empty filter
/// shows the full list unchanged.
#[must_use]
pub fn score(needle: &str, haystack: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let needle: Vec<char> = needle.chars().map(|c| c.to_ascii_lowercase()).collect();
    let hay: Vec<char> = haystack.chars().map(|c| c.to_ascii_lowercase()).collect();

    let mut total = 0i32;
    let mut ni = 0usize;
    let mut last_hit: Option<usize> = None;

    for (hi, &hc) in hay.iter().enumerate() {
        if ni < needle.len() && hc == needle[ni] {
            total += 1;
            if last_hit.is_some_and(|last| last + 1 == hi) {
                // Consecutive runs outrank word-boundary hits so a literal
                // substring beats the same letters scattered across words.
                total += 4;
            }
            let at_boundary = hi == 0 || matches!(hay[hi - 1], ' ' | '-' | '_' | '/' | '.' | ':');
            if at_boundary {
                total += 3; // start of a word
            }
            last_hit = Some(hi);
            ni += 1;
        }
    }

    // Slight penalty for long haystacks so tight matches rank first.
    let length_penalty = i32::try_from(hay.len()).unwrap_or(i32::MAX) / 16;
    (ni == needle.len()).then_some(total - length_penalty)
}

#[cfg(test)]
#[path = "tests/fuzzy.rs"]
mod tests;
