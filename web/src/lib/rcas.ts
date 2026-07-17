// The build-time data layer: reads the RCA workspaces, keeps only the ones
// flagged `published = true`, and turns their client-safe sections into
// rendered HTML for the static site. Runs in Node during `astro build`;
// never ships to the browser.
//
// Source of truth is the same `rcas/` the TUI and desktop read — set
// BEAGLE_RCAS_DIR to point elsewhere (e.g. an oncall checkout); it defaults
// to the repo's `rcas/` one level up from `web/`.

import fs from "node:fs";
import path from "node:path";

import { marked } from "marked";
import { parse as parseToml } from "smol-toml";

import { ansiToHtml } from "./ansi";
import { plainLead, renderMarkdown } from "./render";

const RCAS_DIR = process.env.BEAGLE_RCAS_DIR
  ? path.resolve(process.env.BEAGLE_RCAS_DIR)
  : path.resolve(process.cwd(), "..", "rcas");

const ARCHIVE_DIR = "archive";

/** Client-safe sections, in public order. Notes, Log, and Final Review are
 *  internal working material and never published. */
const PUBLIC_SECTIONS: readonly { file: string; title: string }[] = [
  { file: "timeline.md", title: "Timeline" },
  { file: "root-cause.md", title: "What happened" },
  { file: "impact.md", title: "Impact" },
  { file: "remediation.md", title: "Resolution" },
];

export interface RenderedSection {
  title: string;
  html: string;
}

export interface Diagram {
  name: string;
  /** ANSI colors converted to styled HTML spans; safe for `set:html`. */
  html: string;
}

export interface Incident {
  slug: string;
  title: string;
  severity: string;
  status: string;
  created: string;
  publishedAt: string | null;
  systems: string[];
  /** One-paragraph plain-text lead, for the index cards. */
  lead: string;
  /** Rendered summary HTML, shown at the top of the incident page. */
  summaryHtml: string;
  sections: RenderedSection[];
  diagrams: Diagram[];
}

let cache: Incident[] | null = null;

/** Every published incident, newest first. Memoized for the build. */
export function getPublishedIncidents(): Incident[] {
  cache ??= readAll();
  return cache;
}

/** One published incident by slug, or `undefined`. */
export function getIncident(slug: string): Incident | undefined {
  return getPublishedIncidents().find((incident) => incident.slug === slug);
}

function readAll(): Incident[] {
  const dirs = workspaceDirs();
  const incidents: Incident[] = [];
  for (const dir of dirs) {
    const incident = readIncident(dir);
    if (incident !== null) {
      incidents.push(incident);
    }
  }
  incidents.sort((a, b) => sortKey(b).localeCompare(sortKey(a)));
  return incidents;
}

/** Workspace directories: active ones plus anything under `archive/` (a
 *  published incident stays public even after it's archived). */
function workspaceDirs(): string[] {
  if (!fs.existsSync(RCAS_DIR)) {
    return [];
  }
  const dirs: string[] = [];
  for (const entry of fs.readdirSync(RCAS_DIR, { withFileTypes: true })) {
    if (!entry.isDirectory()) {
      continue;
    }
    if (entry.name === ARCHIVE_DIR) {
      const archive = path.join(RCAS_DIR, ARCHIVE_DIR);
      for (const sub of fs.readdirSync(archive, { withFileTypes: true })) {
        if (sub.isDirectory()) {
          dirs.push(path.join(archive, sub.name));
        }
      }
    } else {
      dirs.push(path.join(RCAS_DIR, entry.name));
    }
  }
  return dirs;
}

function readIncident(dir: string): Incident | null {
  const manifestPath = path.join(dir, "rca.toml");
  if (!fs.existsSync(manifestPath)) {
    return null;
  }
  const meta = parseToml(fs.readFileSync(manifestPath, "utf8"));
  if (meta["published"] !== true) {
    return null;
  }

  const summaryRaw = readSection(dir, "summary.md");
  const sections: RenderedSection[] = [];
  for (const section of PUBLIC_SECTIONS) {
    const raw = readSection(dir, section.file);
    if (raw !== null) {
      sections.push({ title: section.title, html: renderMarkdown(marked, raw) });
    }
  }

  return {
    slug: path.basename(dir),
    title: str(meta["title"]),
    severity: str(meta["severity"]),
    status: str(meta["status"]),
    created: str(meta["created"]),
    publishedAt: typeof meta["published_at"] === "string" ? meta["published_at"] : null,
    systems: strArray(meta["systems"]),
    lead: summaryRaw === null ? "" : plainLead(summaryRaw),
    summaryHtml: summaryRaw === null ? "" : renderMarkdown(marked, summaryRaw),
    sections,
    diagrams: readDiagrams(dir),
  };
}

function readSection(dir: string, file: string): string | null {
  const filePath = path.join(dir, file);
  return fs.existsSync(filePath) ? fs.readFileSync(filePath, "utf8") : null;
}

function readDiagrams(dir: string): Diagram[] {
  const diagramsDir = path.join(dir, "diagrams");
  if (!fs.existsSync(diagramsDir)) {
    return [];
  }
  return fs
    .readdirSync(diagramsDir)
    .filter((name) => name.endsWith(".txt"))
    .sort()
    .map((name) => ({
      name,
      html: ansiToHtml(fs.readFileSync(path.join(diagramsDir, name), "utf8")),
    }));
}

function sortKey(incident: Incident): string {
  return incident.publishedAt ?? incident.created;
}

function str(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function strArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((v): v is string => typeof v === "string")
    : [];
}
