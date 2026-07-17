//! The agent hand-off: composing what `beagle handoff` feeds the agent.
//!
//! The process launch itself lives in the binary (it needs the terminal
//! and the config); this module owns the pure, testable part — building
//! the text sent to the agent's stdin from the configured task prompt and
//! the RCA write-up.

/// Builds the text piped to the hand-off agent's stdin: the task prompt
/// (when configured) then the RCA write-up, framed so the agent knows what
/// it is looking at. An empty or whitespace-only prompt is treated as
/// absent.
#[must_use]
pub fn compose_input(slug: &str, prompt: Option<&str>, rca_markdown: &str) -> String {
    let mut out = String::new();
    out.push_str("# Beagle agent hand-off: ");
    out.push_str(slug);
    out.push_str("\n\n");
    if let Some(prompt) = prompt.map(str::trim).filter(|p| !p.is_empty()) {
        out.push_str(prompt);
        out.push_str("\n\n---\n\n");
    }
    out.push_str(
        "The reviewed incident write-up follows. Do the remediation \
                  work it calls for.\n\n",
    );
    out.push_str(rca_markdown.trim_end());
    out.push('\n');
    out
}

#[cfg(test)]
#[path = "tests/handoff.rs"]
mod tests;
