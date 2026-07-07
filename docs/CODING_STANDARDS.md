# Coding Standards

These are the rules for Rust code in this repo. They exist to keep the TUI
**crash-free on hostile input**, **cheap on memory and I/O**, and **honest in
its types**. Enforcement is layered: what the compiler can enforce lives in
`Cargo.toml` `[lints]`; what clippy can enforce runs in CI with `-D warnings`;
the rest is reviewed against this document.

## 1. Type safety

**Make illegal states unrepresentable.** Prefer a type the compiler checks over
a comment the reviewer might.

- **Newtypes over primitives.** An RCA identifier is `RcaId`, not `String`.
  Construction goes through a validating constructor (`RcaId::new`) so that a
  value of the type *is proof* the invariant holds. Never add a
  `pub fn from_unchecked` escape hatch without a written justification.
- **Enums over stringly-typed data.** Severity, status, tabs, and section kinds
  are closed enums with explicit serde renames — never free-form strings that
  get compared with `==` at usage sites.
- **Parse, don't validate.** Deserialize external data (TOML manifests, CLI
  args) into fully-typed structs at the boundary, in one place. Past the
  boundary, functions accept the typed form only. `#[serde(deny_unknown_fields)]`
  on manifests so typos fail loudly at parse time, not silently at render time.
- **Exhaustive matches.** No `_ =>` arm on enums we own. When a variant is
  added, the compiler must point at every site that needs a decision.
- **Narrow visibility.** Fields are private unless a consumer needs them;
  invariant-carrying types expose accessors, not fields. `pub(crate)` before
  `pub`.

## 2. Error handling

- **The library never panics on runtime input.** `unwrap`/`expect`/`panic!`/
  indexing-that-can-fail are lint-warned crate-wide and rejected in review for
  any path reachable from user data. `expect` is acceptable only for true
  invariants (e.g. a regex literal), with a message stating *why it can't fail*.
- **One error type, `thiserror`, with context.** Every I/O error carries the
  path it happened on (`Error::Io { path, source }`). "Permission denied" with
  no path is useless during an incident.
- **Degrade, don't die.** A corrupt `rca.toml` skips that workspace and surfaces
  a warning in the status bar; it does not abort listing the other 40. A missing
  section file renders as an empty-state hint, not an error screen.
- **`Result` all the way up.** Errors propagate with `?` to `main`, which prints
  them to stderr and exits non-zero. No `.ok()` that swallows an error without a
  comment saying why ignoring it is correct.
- **Terminal restore is unconditional.** Raw mode and the alternate screen are
  torn down on *every* exit path, including panics (`ratatui::init`/`restore`
  panic hooks). Leaving a user's shell broken is a P0 bug.

## 3. Memory efficiency

- **Borrow first, `clone` last.** Take `&str`/`&[T]` parameters; return owned
  values only when the callee genuinely produces new data. Every `.clone()` in
  review gets the question "who needs to own this, and why?"
- **Load lazily, cache narrowly, evict.** Listing workspaces reads only the
  small manifests. Section markdown is read when its tab is opened, cached for
  the current workspace, and dropped on switch. Memory scales with what's on
  screen, not with the corpus.
- **Allocate outside the hot loop.** The draw path runs on every keypress:
  reuse buffers, precompute styled `Text` when content changes rather than
  re-parsing markdown per frame, and never allocate in per-cell code.
- **Right-sized collections.** `Vec::with_capacity` when the size is known;
  `iterator` chains over intermediate `Vec`s; no `collect` just to iterate
  again.
- **No `lazy_static`/globals for state.** All state lives in `App` and is passed
  explicitly. Globals hide lifetimes and make tests order-dependent.

## 4. I/O efficiency

- **Every read is buffered and bounded.** Files are read via
  `fs::read_to_string` (single syscall for small files) with a size cap
  (`MAX_SECTION_BYTES`) checked from metadata first — a runaway 2 GB log pasted
  into a section must not OOM the TUI.
- **Event-driven, never polled.** Filesystem changes arrive via `notify`
  watcher events on a channel; the input loop blocks on `event::poll` with a
  timeout. Idle CPU is ~0%. No `loop { sleep; re-read dir }`.
- **Coalesce filesystem events.** Editors and LLMs write in bursts; drain the
  watcher channel and reload once per drain, not once per event.
- **Write atomically.** Scaffolding writes to a temp file in the target
  directory then renames, so a half-written manifest is never observed by a
  concurrently running TUI.
- **Touch disk only when told.** Reads happen at startup, on watcher events,
  and on explicit reload — never inside the draw loop.

## 5. Concurrency

- Threads communicate over `std::sync::mpsc` channels; no shared mutable state,
  no `Mutex` unless a channel genuinely can't express it (justify in the PR).
- The watcher callback does the minimum (send an event) — no I/O, no parsing,
  no allocation beyond the message.
- No `async`. This is a small event-loop program; an async runtime buys nothing
  and costs compile time, binary size, and stack-trace clarity. Revisit only if
  we add network sources.

## 6. Lints and formatting

Configured in `Cargo.toml` and enforced by CI (`-D warnings`):

- `unsafe_code = "forbid"`
- `clippy::pedantic` (warn, so locally visible; CI denies)
- `clippy::unwrap_used`, `clippy::expect_used` — see §2 for the narrow exception
- `missing_docs` — every public item has a doc comment that says something the
  signature doesn't

`rustfmt` with the repo's `rustfmt.toml` is the only accepted formatting;
never hand-format. `#[allow(...)]` requires an adjacent comment explaining why,
and is scoped to the smallest item possible — never module- or crate-wide.

## 7. Dependencies

Current direct dependencies and why they earn their place:

| Crate | Why |
|---|---|
| `ratatui` + `crossterm` | The TUI. Industry standard, actively maintained. |
| `serde` + `toml` | Manifest (de)serialization at the boundary. |
| `notify` | Cross-platform filesystem watching (FSEvents/inotify/etc.). |
| `thiserror` | Zero-cost derive for the error enum. |
| `time` | Typed timestamps with RFC 3339 serde. |
| `tempfile` (dev) | Filesystem tests in isolated temp dirs. |

Anything new must beat "we write the 50 lines ourselves" on correctness, not
convenience. No `anyhow` (we want a closed error type), no `clap` (arg surface
is tiny; hand-rolled parser is ~80 audited lines), no async runtimes (§5).

## 8. Naming and structure

- Modules by responsibility: `model` (types + invariants), `store` (disk ⇄
  model), `markdown` (text → styled `Text`), `ui` (state machine + drawing).
  Dependencies point one way: `ui → store → model`; `model` imports nothing
  from the others.
- Functions that can fail return `Result` and are named for what they do, not
  how (`read_section`, not `try_get_section_file_contents`).
- Tests live in `#[cfg(test)] mod tests` next to the code they test;
  cross-module behavior tests go in `tests/`.
- Doc comments explain *contract and edge cases* ("returns `Ok(None)` if the
  section file does not exist"), not implementation.
