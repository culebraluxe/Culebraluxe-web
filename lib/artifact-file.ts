// ---------------------------------------------------------------------------
// ARTIFACT FILES — generated files written only when their content changed.
//
// One implementation, used by the manifest CLI, the decision mirror and the learn loop. It was a local
// helper in `scripts/forge-manifest.ts` first and got copied once already; the third caller is where a
// copy becomes a class of bug, so it moved here.
//
// Why "only when changed" matters: these files are committed, and a writer that rewrites a byte-identical
// file leaves a dirty worktree and a meaningless diff in every release. An idempotent writer makes a re-run
// free, which is what lets a gate re-run the generator to compare.
// ---------------------------------------------------------------------------

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname } from 'node:path'

/** Write only when the content differs. Returns true when the file was written. */
export function writeIfChanged(path: string, content: string): boolean {
  if (existsSync(path) && readFileSync(path, 'utf8') === content) return false
  mkdirSync(dirname(path), { recursive: true })
  writeFileSync(path, content, 'utf8')
  return true
}
