#!/usr/bin/env node
// ---------------------------------------------------------------------------
// DO NOT BREAK WHAT IS ALREADY THERE — as a gate, not a poster.
//
// The captain, 2026-09-18: "we should have a naming convention guide and a do not touch file list or
// something ... it's so obvious to me, don't break something that is already there." A list alone is
// not enough because the damage is not malicious, it is mechanical: a tool that writes a file it does
// not understand, a generator whose output path collides with a hand-written table, a rename that takes
// the wrong sibling. On 2026-09-18 `pnpm forge:manifest COLUMN-WRITER-AUDIT` overwrote a 128-column
// audit table with a 34-row index skeleton, and only git brought it back.
//
// So this is a CANARY LIST: each protected file names the marker its first line must keep, and
// `scripts/protected-files.test.ts` fails the harness the moment one loses it. A file that disappears
// or is overwritten by the wrong generator is caught at the next gate instead of weeks later by a
// human who notices a table got shorter.
//
// WHAT IT IS NOT: a permission system. It does not stop a writer, it makes the loss LOUD — and the
// loudness is what turns "somebody should be careful" into a rule the machine enforces.
//
// Adding a file here is a promise that its first line is load-bearing. Removing one requires editing
// the list deliberately, which is the point: the list is the conversation.
// ---------------------------------------------------------------------------

import { existsSync, readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'

export type ProtectedFile = {
  path: string
  /** The first line the file must keep. */
  marker: string
  /** Why it is protected, in the words a future agent needs. */
  why: string
}

export const PROTECTED_FILES: ProtectedFile[] = [
  {
    path: 'AGENTS.md',
    marker: '# CulebraLuxe Agent Operating Context',
    why: 'the always-read handbook every lane is handed; a lane that rewrites it rewrites its own rules',
  },
  {
    path: 'docs/agent/ORIENTATION.md',
    marker: '# ORIENTATION — the map (start here)',
    why: 'the entry point that says where everything else lives',
  },
  {
    path: 'docs/agent/MEMORY.md',
    marker: '# Decision log',
    why: 'the decision log: history is appended, never rewritten (a decision edited away is a decision we will make again)',
  },
  {
    path: 'docs/agent/CURRENT.md',
    marker: '# Current Machine — Forge SDLC',
    why: 'the current-state claim the workshop reads first',
  },
  {
    path: 'docs/agent/releases.md',
    marker: '# Releases — what was actually built, deployed and probed',
    why: 'the release receipts: the only durable evidence of what PROD served and when. Rows are append-only and an eligible receipt is never rewritten',
  },
  {
    path: 'docs/agent/COLUMN-WRITER-AUDIT.md',
    marker: '# Column writer audit',
    why: 'the classified column table. Nearly destroyed on 2026-09-18 by a tool given its name — the reason this list exists',
  },
  {
    path: 'docs/agent/typesafe-failure-triage.md',
    marker: '# TypeSafe failure-triage pilot',
    why: 'an outside judgment source is only safe while its BOUNDARIES are written down: advisory only, a sensor never an oracle, the graduation bar, and where it must never go. Losing this file would leave the tool without its leash',
  },
  {
    path: 'docs/agent/PERIMETER.md',
    marker: '# The perimeter: instruments, what they found, and the traps in running them',
    why: 'the record of the 2026-09-03/04 credential exposure, the measured findings each instrument produced, and the invocations that silently do not cover. Re-deriving this costs a day and the traps do not announce themselves',
  },
]

export type ProtectedDirectory = {
  dir: string
  marker: string
  why: string
}

export const PROTECTED_DIRECTORIES: ProtectedDirectory[] = [
  {
    dir: 'docs/agent/manifest',
    marker: '# Scope manifest',
    why: 'EVERY *.md here is a generated manifest (the freshness gate renders a fresh one per name), so anything else written here can never satisfy that gate — move the artifact out instead',
  },
]

/**
 * The refusal for one file, or null when it is intact. Missing counts as broken: a protected file that
 * is gone is the loudest failure of all, and silence about it would be the bug.
 */
export function markerRefusal(
  path: string,
  content: string | null | undefined,
  marker: string,
): string | null {
  if (content === null || content === undefined) {
    return `${path} is MISSING — it is protected, so its absence is a break, not a cleanup`
  }
  if (!content.startsWith(marker)) {
    const firstLine = content.split(/\r?\n/, 1)[0] ?? ''
    return (
      `${path} no longer starts with "${marker}" (first line: ${JSON.stringify(firstLine.slice(0, 80))})` +
      ' — it was written by something that did not know what it was'
    )
  }
  return null
}

/** Every refusal the given root produces. Empty means every protected file is intact. */
export function protectionRefusals(root: string): string[] {
  const refusals: string[] = []
  for (const file of PROTECTED_FILES) {
    const full = join(root, file.path)
    const content = existsSync(full) ? readFileSync(full, 'utf8') : null
    const refusal = markerRefusal(file.path, content, file.marker)
    if (refusal) refusals.push(`${refusal}\n  why it is protected: ${file.why}`)
  }
  for (const dir of PROTECTED_DIRECTORIES) {
    const full = join(root, dir.dir)
    if (!existsSync(full)) {
      refusals.push(`${dir.dir} is MISSING — ${dir.why}`)
      continue
    }
    for (const name of readdirSync(full).filter((n) => n.endsWith('.md'))) {
      const path = `${dir.dir}/${name}`
      const refusal = markerRefusal(path, readFileSync(join(root, path), 'utf8'), dir.marker)
      if (refusal) refusals.push(`${refusal}\n  why it matters here: ${dir.why}`)
    }
  }
  return refusals
}

