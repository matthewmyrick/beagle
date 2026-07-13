//! Related-incident ranking: which past RCAs look like this one?
//!
//! The workspace archive is institutional memory — "how did alloy break
//! last time?" is usually the fastest path to a root cause. Ranking is
//! deliberately simple and runs entirely over the manifests already in
//! memory: shared `systems` weigh heaviest (same service breaking again),
//! shared `tags` add signal (same failure class), newest first on ties.

use crate::model::RcaSummary;

/// Weight of one shared system vs. one shared tag.
const SYSTEM_WEIGHT: u32 = 3;
const TAG_WEIGHT: u32 = 1;

/// One related workspace, with why it ranked.
#[derive(Debug, Clone, PartialEq)]
pub struct Related<'a> {
    /// Relevance score (shared systems × 3 + shared tags × 1).
    pub score: u32,
    /// The systems this workspace shares with the target.
    pub shared_systems: Vec<String>,
    /// The tags this workspace shares with the target.
    pub shared_tags: Vec<String>,
    /// The related workspace itself.
    pub rca: &'a RcaSummary,
}

/// Ranks every other workspace by similarity to `target`: highest score
/// first, newest first on ties. Workspaces sharing nothing (score 0) are
/// omitted — an unrelated incident is noise, not a weak signal.
#[must_use]
pub fn rank<'a>(target: &RcaSummary, all: &'a [RcaSummary]) -> Vec<Related<'a>> {
    let mut related: Vec<Related<'a>> = all
        .iter()
        .filter(|candidate| candidate.id != target.id)
        .filter_map(|candidate| {
            let shared_systems = intersect(&target.meta.systems, &candidate.meta.systems);
            let shared_tags = intersect(&target.meta.tags, &candidate.meta.tags);
            let score = u32::try_from(shared_systems.len()).unwrap_or(u32::MAX) * SYSTEM_WEIGHT
                + u32::try_from(shared_tags.len()).unwrap_or(u32::MAX) * TAG_WEIGHT;
            (score > 0).then_some(Related {
                score,
                shared_systems,
                shared_tags,
                rca: candidate,
            })
        })
        .collect();
    related.sort_by_key(|r| {
        (
            std::cmp::Reverse(r.score),
            std::cmp::Reverse(r.rca.meta.created.unix_timestamp()),
        )
    });
    related
}

/// Values present in both lists (case-insensitive), in `a`'s order.
fn intersect(a: &[String], b: &[String]) -> Vec<String> {
    a.iter()
        .filter(|value| b.iter().any(|other| other.eq_ignore_ascii_case(value)))
        .cloned()
        .collect()
}

/// A short human label for why something ranked: `2 systems, 1 tag`.
#[must_use]
pub fn shared_label(related: &Related<'_>) -> String {
    let mut parts = Vec::new();
    match related.shared_systems.len() {
        0 => {}
        1 => parts.push(format!("system: {}", related.shared_systems[0])),
        n => parts.push(format!("{n} systems")),
    }
    match related.shared_tags.len() {
        0 => {}
        1 => parts.push(format!("tag: {}", related.shared_tags[0])),
        n => parts.push(format!("{n} tags")),
    }
    parts.join(", ")
}

#[cfg(test)]
#[path = "tests/similar.rs"]
mod tests;
