// Typed wrappers over the Tauri invoke surface. Every command the
// frontend calls lives here — components never import `invoke` directly,
// so the IPC surface stays greppable in one file.

import { invoke } from "@tauri-apps/api/core";

import type { Listing } from "./types";

/** Active + archived workspaces plus load problems, sidebar-sorted. */
export async function listWorkspaces(): Promise<Listing> {
  return invoke<Listing>("list_workspaces");
}

/**
 * One section's raw markdown, or `null` when the file doesn't exist yet
 * (a normal state — the investigation just hasn't got there).
 */
export async function readSection(id: string, file: string): Promise<string | null> {
  return invoke<string | null>("read_section", { id, file });
}

/** The workspace's diagram file names, sorted. */
export async function listDiagrams(id: string): Promise<string[]> {
  return invoke<string[]>("list_diagrams", { id });
}

/** One diagram's text, ANSI-stripped; `null` if it vanished. */
export async function readDiagram(id: string, name: string): Promise<string | null> {
  return invoke<string | null>("read_diagram", { id, name });
}
