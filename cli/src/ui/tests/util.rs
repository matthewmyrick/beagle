//! Shared fixtures for the `ui` test modules: a scaffolded app and a
//! synthetic keypress.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::model::{RcaId, Severity};
use crate::store::{new_meta, Store};

use super::keys::Flow;
use super::App;

/// An app over a fresh temp store scaffolded with `n` workspaces.
pub(crate) fn app_with(n: usize) -> App {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("store");
    for i in 0..n {
        let id = RcaId::new(format!("rca-{i}")).expect("valid id");
        store
            .scaffold(&id, &new_meta(format!("RCA {i}"), Severity::Medium))
            .expect("scaffold");
    }
    // Leak the tempdir handle so the files outlive the test setup; the OS
    // cleans the temp root. Simpler than threading the guard through App.
    std::mem::forget(tmp);
    App::new(store).expect("app")
}

/// Feeds one unmodified keypress through the app's key handler.
pub(crate) fn press(app: &mut App, code: KeyCode) -> Flow {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
}
