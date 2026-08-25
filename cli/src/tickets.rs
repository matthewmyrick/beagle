//! Discovering the PRs linked to an attached tracking ticket, for
//! `beagle ticket sync`.
//!
//! Like [`crate::prs`], nothing here adds a dependency: it shells out to
//! `gh` (GitHub CLI mode) or `curl` (any API mode / Linear), parsing the
//! JSON with the `serde_json` already in the tree. The parsers are pure
//! and unit-tested; the command invocation is a thin wrapper.

use std::process::Command;

use crate::config::{AccessMode, PlatformConfig, TicketsConfig};
use crate::error::{Error, Result};
use crate::model::TicketPlatform;

/// The PR URLs linked to `reference` (a Linear or GitHub issue), per the
/// configured access mode for its platform. An empty list means "none
/// found"; an `Err` means the lookup itself failed (tool missing, network,
/// unparseable output).
///
/// # Errors
/// [`Error::Tool`] when the reference is not a recognised ticket, the
/// platform tool fails, or its output cannot be parsed.
pub fn linked_prs(reference: &str, cfg: Option<&TicketsConfig>) -> Result<Vec<String>> {
    let platform = TicketPlatform::of(reference).ok_or_else(|| tool_err("not a ticket"))?;
    match platform {
        TicketPlatform::GitHub => github_linked_prs(reference, platform_cfg(cfg, platform)),
        TicketPlatform::Linear => linear_linked_prs(reference, platform_cfg(cfg, platform)),
    }
}

fn platform_cfg(cfg: Option<&TicketsConfig>, platform: TicketPlatform) -> PlatformConfig {
    cfg.and_then(|c| match platform {
        TicketPlatform::GitHub => c.github.clone(),
        TicketPlatform::Linear => c.linear.clone(),
    })
    .unwrap_or_default()
}

fn tool_err(message: impl Into<String>) -> Error {
    Error::Tool {
        tool: "ticket sync",
        message: message.into(),
    }
}

// ---- GitHub -------------------------------------------------------------

/// `(owner, repo, number)` parsed from a GitHub issue URL.
#[must_use]
pub fn github_issue_parts(url: &str) -> Option<(String, String, String)> {
    let rest = url.split("github.com/").nth(1)?;
    let mut segs = rest.split('/');
    let owner = segs.next()?.to_owned();
    let repo = segs.next()?.to_owned();
    if segs.next()? != "issues" {
        return None;
    }
    let number: String = segs
        .next()?
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    (!owner.is_empty() && !repo.is_empty() && !number.is_empty()).then_some((owner, repo, number))
}

/// Collects the PR URLs from a GitHub issue **timeline** JSON array: every
/// `cross-referenced` event whose source is a pull request.
#[must_use]
pub fn github_timeline_prs(json: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let mut prs = Vec::new();
    for event in value.as_array().into_iter().flatten() {
        if event.get("event").and_then(serde_json::Value::as_str) != Some("cross-referenced") {
            continue;
        }
        let source = event.get("source").and_then(|s| s.get("issue"));
        let Some(issue) = source else { continue };
        // A cross-reference from a *pull request* carries a `pull_request`
        // object; its `html_url` is the PR.
        if issue.get("pull_request").is_some() {
            if let Some(url) = issue.get("html_url").and_then(serde_json::Value::as_str) {
                let url = url.to_owned();
                if !prs.contains(&url) {
                    prs.push(url);
                }
            }
        }
    }
    prs
}

fn github_linked_prs(url: &str, cfg: PlatformConfig) -> Result<Vec<String>> {
    let (owner, repo, number) = github_issue_parts(url)
        .ok_or_else(|| tool_err(format!("`{url}` is not a GitHub issue")))?;
    let path = format!("repos/{owner}/{repo}/issues/{number}/timeline");
    let json = match cfg.mode {
        AccessMode::Cli => run(
            "gh",
            &[
                "api",
                &path,
                "--paginate",
                "-H",
                "Accept: application/vnd.github+json",
            ],
        )?,
        AccessMode::Api => {
            let token = cfg
                .token
                .ok_or_else(|| tool_err("github api mode needs a `token` in [tickets.github]"))?;
            run(
                "curl",
                &[
                    "-sS",
                    "-H",
                    &format!("Authorization: Bearer {token}"),
                    "-H",
                    "Accept: application/vnd.github+json",
                    &format!("https://api.github.com/{path}?per_page=100"),
                ],
            )?
        }
    };
    Ok(github_timeline_prs(&json))
}

// ---- Linear -------------------------------------------------------------

/// The `ABC-123` identifier from a Linear URL or bare id.
#[must_use]
pub fn linear_issue_id(reference: &str) -> Option<String> {
    let r = reference.trim();
    // From a URL: the `.../issue/ABC-123/...` segment.
    if let Some(rest) = r.split("/issue/").nth(1) {
        let id = rest.split('/').next().unwrap_or("");
        if is_linear_id(id) {
            return Some(id.to_owned());
        }
    }
    is_linear_id(r).then(|| r.to_owned())
}

fn is_linear_id(s: &str) -> bool {
    let mut parts = s.splitn(2, '-');
    matches!((parts.next(), parts.next()), (Some(team), Some(num))
        if !team.is_empty()
            && team.chars().all(|c| c.is_ascii_alphabetic())
            && !num.is_empty()
            && num.chars().all(|c| c.is_ascii_digit()))
}

/// Collects PR URLs from a Linear GraphQL `issue.attachments` response:
/// attachment URLs that point at a GitHub (or GitLab) pull/merge request.
#[must_use]
pub fn linear_attachment_prs(json: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let nodes = value
        .get("data")
        .and_then(|d| d.get("issue"))
        .and_then(|i| i.get("attachments"))
        .and_then(|a| a.get("nodes"));
    let mut prs = Vec::new();
    for node in nodes
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(url) = node.get("url").and_then(serde_json::Value::as_str) {
            if url.contains("/pull/") || url.contains("/merge_requests/") {
                let url = url.to_owned();
                if !prs.contains(&url) {
                    prs.push(url);
                }
            }
        }
    }
    prs
}

fn linear_linked_prs(reference: &str, cfg: PlatformConfig) -> Result<Vec<String>> {
    let id = linear_issue_id(reference)
        .ok_or_else(|| tool_err(format!("could not read a Linear id from `{reference}`")))?;
    let query = format!(
        r#"{{"query":"query {{ issue(id: \"{id}\") {{ attachments {{ nodes {{ url }} }} }} }}"}}"#
    );
    let token = match cfg.mode {
        AccessMode::Api => cfg
            .token
            .ok_or_else(|| tool_err("linear api mode needs a `token` in [tickets.linear]"))?,
        // The Linear CLI has no stable JSON contract for linked PRs; the
        // API path is the supported one. A key is still required.
        AccessMode::Cli => cfg.token.ok_or_else(|| {
            tool_err("linear needs `mode = \"api\"` and a `token` in [tickets.linear]")
        })?,
    };
    let json = run(
        "curl",
        &[
            "-sS",
            "-X",
            "POST",
            "https://api.linear.app/graphql",
            "-H",
            &format!("Authorization: {token}"),
            "-H",
            "Content-Type: application/json",
            "-d",
            &query,
        ],
    )?;
    Ok(linear_attachment_prs(&json))
}

// ---- shell --------------------------------------------------------------

/// Runs `cmd args`, returning stdout. Errors if the binary is missing or
/// the command exits non-zero.
fn run(cmd: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| tool_err(format!("`{cmd}` failed to launch: {e} (is it installed?)")))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(tool_err(format!("`{cmd}` failed: {}", err.trim())));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
#[path = "tests/tickets.rs"]
mod tests;
