# beagle web

The public postmortem site: a static Astro build that turns **published**
RCA workspaces into a sleek, client-facing incident history. Read-only —
the TUI and desktop app do the investigating; this presents the result.

## How it works

`astro build` reads the `rcas/` workspaces at build time (see
`src/lib/rcas.ts`), keeps only incidents flagged `published = true` in
their `rca.toml`, and renders their **client-safe** sections — Summary,
Timeline, Root Cause, Impact, Resolution, and Diagrams. Notes, Log, and
Final Review are internal and never published. Output is a self-contained
static site in `dist/` you can deploy anywhere.

Publish an incident from the CLI:

```sh
beagle publish <slug>     # sets published = true, stamps published_at
beagle unpublish <slug>   # makes it private again
```

## Develop

```sh
cd web
npm install
npm run dev          # reads ../rcas by default
```

Point it at a different store with `BEAGLE_RCAS_DIR`:

```sh
BEAGLE_RCAS_DIR=~/oncall/rcas npm run build
```

## Quality gate (CI runs this)

```sh
npm run check        # prettier + astro check (types) + vitest
```

## Deploy

`npm run build` emits `dist/`. Deploy it to Vercel, Netlify, GitHub
Pages, S3 — any static host. Set `BEAGLE_SITE_URL` to your origin for
correct absolute URLs.

## Layout

```text
src/lib/rcas.ts    build-time reader: parse rca.toml, filter published, render
src/lib/*.ts       pure helpers (format, text, render) + vitest tests
src/pages/         index (history) + incidents/[slug] (one postmortem)
src/components/    presentational Astro components
src/styles/        the public design system
```
