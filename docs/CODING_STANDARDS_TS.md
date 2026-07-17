# TypeScript coding standards (desktop & web)

The Rust crate's bar — no panics on user data, pedantic lints at zero
warnings, graceful degradation on any input — exists because beagle is
used *during incidents*. The TypeScript frontends inherit that context: a
blank screen while someone reads an incident write-up is as unacceptable
in a webview as a crash is in a terminal. These are the TS translations
of the rules in `CODING_STANDARDS.md`; CI enforces them (`npm run check`
must pass, which runs prettier, ESLint with `--max-warnings 0`, `tsc`,
and vitest).

## 1. Strictness is not negotiable

- `tsconfig.json` runs `strict` plus `noUncheckedIndexedAccess`,
  `exactOptionalPropertyTypes`, `noImplicitReturns`,
  `verbatimModuleSyntax`, and friends. **Never weaken a compiler flag to
  make an error go away** — fix the code. New flags may be added; none
  removed.
- ESLint runs typescript-eslint's `strictTypeChecked` +
  `stylisticTypeChecked` with zero warnings tolerated. `any`, non-null
  assertions, and unsafe casts are all errors. If you need an escape
  hatch, you almost certainly need a type guard instead.
- Every exported function declares its return type
  (`explicit-module-boundary-types`).

## 2. Files stay small

`max-lines` (300, excluding blanks/comments) and `max-lines-per-function`
(120) are **errors**. This is deliberate pressure to split: one component
per file, pure logic out of components and into `src/lib/`, the IPC
surface isolated in `src/api.ts`. If a file fights the limit, it has two
responsibilities — split it, don't raise the limit.

## 3. Structure

```text
src/
  types.ts        the IPC contract (mirrors src-tauri/src/dto.rs — keep in sync)
  api.ts          every invoke() wrapper; components never call invoke directly
  lib/            pure, framework-free logic — this is where tests live easiest
  components/     one presentational component per file
  App.tsx         the composition root; state lives here or in hooks
```

- **Parse, don't validate** at the boundary: data crossing IPC is typed
  in `types.ts`; anything looser than the contract gets narrowed with a
  type guard (see `lib/format.ts`), never a cast.
- Rust owns the domain. Sorting, filtering, format parsing, and lifecycle
  rules live in the `cli/` crate and cross the boundary as data. The
  frontend renders; it does not reimplement.

## 4. Degrade, never crash

- Unknown enum values from a newer backend render neutrally (see
  `severityColor`), never throw.
- A failed command shows an error banner and leaves the rest of the UI
  usable. `catch` on every promise chain; no floating promises (enforced
  by lint).
- Absent sections are a normal state with a hint, not an error.

## 5. Tests

- Pure logic in `src/lib/` gets unit tests as a matter of course.
- Components get render tests (`@testing-library/react`) for behavior:
  what's shown, what fires on interaction. No snapshot tests — they
  assert everything and therefore nothing.
- The `npm run check` suite is the merge gate; it runs in CI on every
  desktop PR (`.github/workflows/desktop.yml`).

## 6. Dependencies

The Rust crate ships with seven dependencies on purpose. npm makes that
discipline harder, so the rule is explicit: **a new runtime dependency
needs a reason in the PR description** — what it does, why ~50 lines of
our own code can't, and its transitive weight. Dev-tooling additions are
freer, but the runtime bundle is part of the product.

## 7. The Rust side of desktop/

`desktop/src-tauri` follows `CODING_STANDARDS.md` (the Rust rules): no
`unsafe`, pedantic clippy at `-D warnings`, `unwrap_used`/`expect_used`
warned. Commands return `Result<_, String>` — errors are data for the
frontend's banner, never process exits.

## 8. The web app (`web/`, Astro)

The public postmortem site follows the same spirit as the frontends
above, adapted to Astro + SSG:

- **Read-only and public.** The site never writes; it renders **published**
  RCAs only (`published = true` in `rca.toml`) and only their client-safe
  sections (Summary, Timeline, Root Cause, Impact, Resolution, Diagrams).
  Notes, Log, and Final Review are internal and never leave the machine.
- **Build-time only.** All `rcas/` reading, TOML parsing, and markdown
  rendering happen in `src/lib/` during `astro build` — nothing ships to
  the browser but static HTML/CSS. Keep it that way; this is a content
  site, not an app.
- **Pure `lib/`, tested.** `format`, `text`, `render` are pure and unit
  tested with vitest; the build-time reader (`rcas.ts`) composes them.
- **Same strictness knobs** (`strictNullChecks`, `noUncheckedIndexedAccess`,
  `verbatimModuleSyntax`) via `astro/tsconfigs/strict`; `npm run check`
  (prettier + `astro check` + vitest) is the gate, enforced in CI.
