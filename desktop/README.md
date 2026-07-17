# beagle desktop

The desktop viewer for RCA workspaces: a Tauri 2 shell whose Rust backend
reuses the `cli/` crate's domain layer (store, model, markdown, similar)
and whose React frontend renders it. The on-disk format under `rcas/` is
the shared API — this app is a second renderer, not a second
implementation.

## Develop

```sh
cd desktop
npm install
npm run tauri dev     # opens the app; finds the nearest rcas/ above the cwd
```

Root resolution: config file `root` (`~/.config/beagle/config.toml`) →
nearest ancestor directory containing `rcas/` → the working directory.

## Quality gate (CI runs exactly this)

```sh
npm run check         # prettier + eslint (zero warnings) + tsc + vitest
cd src-tauri && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings
```

Read `../docs/CODING_STANDARDS_TS.md` before contributing — strictness
flags and file-size limits are enforced, not advisory.

## Layout

```text
src/types.ts     IPC contract (mirrors src-tauri/src/dto.rs)
src/api.ts       all invoke() wrappers
src/lib/         pure logic + unit tests
src/components/  one component per file + render tests
src-tauri/       Rust: commands.rs (IPC surface), dto.rs (contract)
```
