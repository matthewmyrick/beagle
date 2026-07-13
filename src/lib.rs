//! `beagle` — a terminal UI for root-cause analysis (RCA) workspaces.
//!
//! Each debugged system gets a workspace under `rcas/<id>/` containing a TOML
//! manifest plus markdown sections and ASCII diagrams. External tools
//! (typically Claude) write those files; this crate renders them as a tabbed
//! TUI so a human can understand what broke, why it broke, and how to fix it.
//!
//! Module layering (dependencies point strictly downward):
//!
//! ```text
//! ui ──▶ store ──▶ model
//!  │                 ▲
//!  ├──▶ markdown ────┘
//!  └──▶ ansi
//! ```
//!
//! `config` (the user config file) and `update` (self-update against GitHub
//! releases) sit beside `ui` at the top of the stack; both depend only on
//! `error`.

pub mod ansi;
pub mod banner;
pub mod clipboard;
pub mod config;
pub mod error;
pub mod fuzzy;
pub mod links;
pub mod markdown;
pub mod model;
pub mod prs;
pub mod similar;
pub mod skills;
pub mod store;
pub mod ui;
pub mod update;

pub use error::Error;
