//! The blocking event loop: input with a timeout, watcher events over a
//! channel, and burst-coalesced reloads. Idle CPU is ~0%.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use notify::{RecursiveMode, Watcher as _};
use ratatui::DefaultTerminal;

use crate::error::Result;
use crate::model::RcaId;
use crate::store::LoadWarning;
#[cfg(doc)]
use crate::Error; // referenced by rustdoc links in this module

use super::keys::Flow;
use super::{view, App};

/// What a reload discovered, for announcements and notifications.
#[derive(Debug, Default)]
pub(crate) struct ReloadDelta {
    /// Titles of workspaces that appeared since the last listing.
    pub arrived: Vec<String>,
    /// `(title, from, to)` status transitions observed on disk.
    pub status_changes: Vec<(String, crate::model::Status, crate::model::Status)>,
}

impl App {
    /// Runs the event loop until the user quits.
    ///
    /// Blocks on input with a timeout; filesystem changes arrive over a
    /// channel from the `notify` watcher, so idle CPU is ~0%.
    ///
    /// # Errors
    /// Returns [`Error::Terminal`] on draw/poll failures or [`Error::Watch`]
    /// if the watcher cannot be attached.
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let (tx, rx) = mpsc::channel::<()>();
        // The callback does the minimum: signal "something changed".
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if res.is_ok() {
                    let _ = tx.send(()); // receiver gone means we are shutting down
                }
            })?;
        watcher.watch(self.store.watch_root(), RecursiveMode::Recursive)?;

        // PR merge-status polling: a background thread queries `gh` (every
        // 30 min, plus whenever the attached-PR set changes) and reports
        // over a channel, so the UI thread never blocks on the network.
        // Without gh, the poller exits and PRs stay plain links.
        let (urls_tx, urls_rx) = mpsc::channel::<Vec<String>>();
        let (states_tx, states_rx) = mpsc::channel();
        crate::prs::spawn_poller(urls_rx, states_tx);
        let mut polled_urls = self.pr_urls();
        let _ = urls_tx.send(polled_urls.clone());

        loop {
            while let Ok(states) = states_rx.try_recv() {
                self.pr_states.extend(states);
            }
            self.advance_merged_reviews();
            if drain(&rx) {
                let delta = self.reload();
                self.status = Some(match delta.arrived.first() {
                    Some(title) => format!("new incident: {title}"),
                    None => "reloaded (files changed on disk)".to_owned(),
                });
                if self.follow {
                    // Tail-f: pin the current tab to its (possibly longer)
                    // bottom; the draw pass clamps to the real height.
                    self.scroll = u16::MAX;
                }
                let urls = self.pr_urls();
                if urls != polled_urls {
                    polled_urls.clone_from(&urls);
                    let _ = urls_tx.send(urls);
                }
            }
            self.ensure_pane();
            // The loop turns at least every 250 ms (the poll timeout below),
            // so bumping the counter here animates the investigating spinner
            // at ~4 fps without any extra wakeups.
            self.tick = self.tick.wrapping_add(1);
            terminal.draw(|frame| view::draw(frame, &mut self))?;

            if event::poll(Duration::from_millis(250))? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        match self.handle_key(key) {
                            Flow::Quit => return Ok(()),
                            Flow::Edit(path) => self.open_in_editor(terminal, &path),
                            Flow::Continue => {}
                        }
                    }
                    Event::Mouse(mouse) => self.handle_mouse(mouse),
                    // Resize triggers a redraw on the next loop turn anyway.
                    _ => {}
                }
            }
        }
    }

    /// `E`: suspend the TUI, open `path` in the user's editor (same
    /// resolution as `beagle config`: config `editor` → `$VISUAL` →
    /// `$EDITOR` → vim), and restore. Editor failures land in the status
    /// bar — a typoed `$EDITOR` must never take the TUI down. The user's
    /// own edit is not flagged unread for them.
    fn open_in_editor(&mut self, terminal: &mut DefaultTerminal, path: &std::path::Path) {
        use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
        use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};

        let editor = crate::config::editor(crate::config::load_default().ok().flatten().as_ref());
        let mut parts = editor.split_whitespace();
        let program = parts.next().unwrap_or("vim").to_owned();
        let args: Vec<&str> = parts.collect();

        let _ = crossterm::execute!(std::io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        let _ = crossterm::terminal::disable_raw_mode();
        let status = std::process::Command::new(&program)
            .args(&args)
            .arg(path)
            .status();
        let _ = crossterm::terminal::enable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture);
        let _ = terminal.clear(); // the editor drew over everything

        let _ = self.reload();
        // The reload flagged the file the user just wrote as unread — but
        // they have quite literally read it. Watcher events queued behind
        // this see an unchanged mtime snapshot, so the flag stays clear.
        if let (Some(kind), Some(rca)) = (self.tab.section(), self.selected_rca()) {
            let key = (rca.id.clone(), kind);
            self.unread.remove(&key);
        }
        self.status = Some(match status {
            Ok(code) if code.success() => format!("edited {}", path.display()),
            Ok(code) => format!("editor `{editor}` exited with {code}"),
            Err(e) => format!("editor `{editor}` failed to launch: {e}"),
        });
    }

    /// `review` or `agent` → `final-review` the moment every attached fix
    /// PR has merged, per the states the background `gh` poller reports —
    /// whether the PR was opened during review or by the agent doing the
    /// remediation. Workspaces tagged `skip-final-review` advance straight
    /// to `finished` instead: no checklist, no sign-off. The manifest
    /// write is atomic and also wakes the filesystem watcher, so other
    /// beagle instances see the transition too. Workspaces without
    /// attached PRs never auto-advance — there is nothing to observe
    /// merge.
    pub(crate) fn advance_merged_reviews(&mut self) {
        use crate::model::Status;
        let ready: Vec<(RcaId, String, Status)> = self
            .rcas
            .iter()
            .filter(|rca| {
                matches!(rca.meta.status, Status::Review | Status::Agent)
                    && !rca.meta.prs.is_empty()
            })
            .filter(|rca| {
                rca.meta
                    .prs
                    .iter()
                    .all(|url| self.pr_states.get(url) == Some(&crate::prs::PrState::Merged))
            })
            .map(|rca| {
                let target = if rca.meta.skips_final_review() {
                    Status::Finished
                } else {
                    Status::FinalReview
                };
                (rca.id.clone(), rca.meta.title.clone(), target)
            })
            .collect();
        if ready.is_empty() {
            return;
        }
        for (id, title, target) in &ready {
            match self.store.set_status(id, *target) {
                Ok(_) => {
                    self.status = Some(match target {
                        Status::Finished => format!(
                            "→ finished: {title} (all fix PRs merged; final review skipped)"
                        ),
                        _ => format!("→ final-review: {title} (all fix PRs merged)"),
                    });
                }
                Err(e) => self
                    .warnings
                    .push(LoadWarning(format!("auto {target} failed for {id}: {e}"))),
            }
        }
        let _ = self.reload();
    }

    /// Whether a transition *to* `status` should fire a notification, per
    /// the configured `[notify_events]` filter.
    fn notifies_for(&self, status: crate::model::Status) -> bool {
        use crate::model::Status;
        let e = &self.notify_events;
        match status {
            Status::Investigating => e.investigating,
            Status::Review => e.review,
            Status::Agent => e.agent,
            Status::FinalReview => e.final_review,
            Status::Finished => e.finished,
        }
    }

    /// Re-lists workspaces, keeping the selection pinned to the same id when
    /// it survives the reload, and drops the content cache. Returns what
    /// changed (new workspaces, status transitions) and — when enabled —
    /// fires desktop notifications for it, so every reload path (watcher,
    /// manual `r`, auto-advance, sign-off) notifies consistently.
    pub(crate) fn reload(&mut self) -> ReloadDelta {
        let previous = self.selected_rca().map(|r| r.id.clone());
        let known: HashMap<RcaId, crate::model::Status> = self
            .rcas
            .iter()
            .map(|r| (r.id.clone(), r.meta.status))
            .collect();
        let mut delta = ReloadDelta::default();
        match self.store.list_all() {
            Ok(listing) => {
                for rca in &listing.summaries {
                    match known.get(&rca.id) {
                        None => delta.arrived.push(rca.meta.title.clone()),
                        Some(&old) if old != rca.meta.status => {
                            delta.status_changes.push((
                                rca.meta.title.clone(),
                                old,
                                rca.meta.status,
                            ));
                        }
                        Some(_) => {}
                    }
                }
                self.rcas = listing.summaries;
                self.warnings = listing.warnings;
                self.broken = listing.broken;
            }
            Err(e) => {
                self.warnings = vec![LoadWarning(format!("reload failed: {e}"))];
            }
        }
        self.refresh_mtimes(true);
        self.recompute_visible(previous);
        self.pane = None;
        if self.notify_enabled {
            if self.notify_events.new_incident {
                for title in &delta.arrived {
                    crate::notify::send("beagle — new incident", title);
                }
            }
            for (title, _, to) in &delta.status_changes {
                if self.notifies_for(*to) {
                    crate::notify::send(&format!("beagle — {to}"), title);
                }
            }
        }
        delta
    }

    /// Re-snapshots every section file's mtime. When `mark_unread` is set,
    /// sections whose mtime advanced (or which newly appeared) since the
    /// last snapshot are flagged unread until viewed.
    pub(crate) fn refresh_mtimes(&mut self, mark_unread: bool) {
        for rca in &self.rcas {
            for (kind, mtime) in self.store.section_mtimes(&rca.id) {
                let key = (rca.id.clone(), kind);
                let changed = match self.mtimes.get(&key) {
                    Some(old) => mtime > *old,
                    None => true, // file appeared since the last snapshot
                };
                if changed {
                    if mark_unread {
                        self.unread.insert(key.clone());
                    }
                    // Re-scan checkboxes only for files that actually
                    // changed — the checklist cache mirrors this snapshot.
                    let stats = self
                        .store
                        .read_section(&rca.id, kind)
                        .ok()
                        .flatten()
                        .map(|content| crate::markdown::checklist_stats(&content));
                    match stats {
                        Some((checked, total)) if total > 0 => {
                            self.checklists.insert(key.clone(), (checked, total));
                        }
                        _ => {
                            self.checklists.remove(&key);
                        }
                    }
                }
                self.mtimes.insert(key, mtime);
            }
        }
    }
}

/// Drains every pending watcher event, returning whether any arrived.
/// Editors and LLMs write in bursts; we reload once per burst, not per event.
fn drain(rx: &Receiver<()>) -> bool {
    let mut any = false;
    loop {
        match rx.try_recv() {
            Ok(()) => any = true,
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return any,
        }
    }
}

#[cfg(test)]
#[path = "tests/event_loop.rs"]
mod tests;
