// @ts-check
import { defineConfig } from "astro/config";

// Static output: `astro build` emits a self-contained site to dist/ that
// deploys anywhere (Vercel, Netlify, Pages, S3) — no server, no database,
// matching beagle's files-in-files-out design. The build reads the RCA
// workspaces directly (see src/lib/rcas.ts).
export default defineConfig({
  output: "static",
  // Set to your deployed origin for correct absolute URLs / sitemaps.
  site: process.env.BEAGLE_SITE_URL ?? "https://example.com",
});
