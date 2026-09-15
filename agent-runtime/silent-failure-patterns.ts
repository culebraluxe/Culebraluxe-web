// ---------------------------------------------------------------------------
// SILENT-FAILURE PATTERNS — the Error Capture Obligation as a gate.
//
// `AGENTS.md` says new server code that can fail MUST route its failure through the durable capture
// framework: no bare `try/catch` that swallows, no `console.error` on its own, no failure that escapes
// uncaptured. That was a sentence, and a sentence does not hold: on 2026-09-15 a `catch` silently
// dropped EVERY error record in one code path while a dead server-action graph hid behind React's
// minified `#441`, and the two together cost hours of inference.
//
// So it becomes a check. It is deliberately a LIBRARY over an explicit file list (not a repo walk):
// Assay hands it the changed files, a NEW hit in the change fails the change, and pre-existing hits
// elsewhere are reported but never block - the difference between "you broke it" and "it was already
// like that", which is the only distinction that makes a gate usable.
// ---------------------------------------------------------------------------

export type SilentFailurePattern =
  | 'empty-catch'
  | 'swallowed-catch'
  | 'console-error-without-capture'
  | 'bare-500-in-catch'

export type SilentFailureHit = {
  path: string
  line: number
  pattern: SilentFailurePattern
  snippet: string
}

export type SilentFailureFile = {
  path: string
  content: string
}

/** Functions that mean "this failure was captured", so a `console.error` beside one is fine. */
const CAPTURE_MARKERS = [
  'captureServerError',
  'captureServerLog',
  'captureError',
  'recordError',
  'withApiHandler',
  'withServerErrorCapture',
]

/** Directories where a swallowed failure is a product defect rather than a script's convenience. */
function isServerSurface(path: string): boolean {
  return /^(app|services)\//.test(path) || path.includes('/app/') || path.includes('/services/')
}

function lineOf(content: string, index: number): number {
  return content.slice(0, index).split('\n').length
}

/**
 * Find the silent-failure patterns in the files handed over. Pure: no filesystem, no repo walking.
 */
export function findSilentFailures(files: readonly SilentFailureFile[]): SilentFailureHit[] {
  const hits: SilentFailureHit[] = []

  for (const file of files) {
    const capturePresent = CAPTURE_MARKERS.some((marker) => file.content.includes(marker))

    // 1. An empty catch block: the failure is deleted, not handled.
    for (const match of file.content.matchAll(/catch\s*(?:\([^)]*\))?\s*\{\s*\}/g)) {
      hits.push({
        path: file.path,
        line: lineOf(file.content, match.index ?? 0),
        pattern: 'empty-catch',
        snippet: match[0].replace(/\s+/g, ' '),
      })
    }

    // 2. A catch that swallows the error into a benign default.
    for (const match of file.content.matchAll(
      /\.catch\(\s*(?:\([^)]*\)|\s*)[\s\S]{0,20}?=>\s*(?:\[\]|null|undefined|0|''|""|\(\{\}\))\s*\)/g,
    )) {
      hits.push({
        path: file.path,
        line: lineOf(file.content, match.index ?? 0),
        pattern: 'swallowed-catch',
        snippet: match[0].replace(/\s+/g, ' '),
      })
    }

    // 3. console.error on a server surface with no capture in the same file.
    if (isServerSurface(file.path) && !capturePresent) {
      for (const match of file.content.matchAll(/console\.error\(/g)) {
        hits.push({
          path: file.path,
          line: lineOf(file.content, match.index ?? 0),
          pattern: 'console-error-without-capture',
          snippet: 'console.error( with no capture in this file',
        })
      }
    }

    // 4. A catch that answers with a bare 500 body and records nothing.
    for (const match of file.content.matchAll(
      /catch\s*(?:\([^)]*\))?\s*\{[\s\S]{0,400}?status:\s*500/g,
    )) {
      const block = match[0]
      if (!CAPTURE_MARKERS.some((marker) => block.includes(marker))) {
        hits.push({
          path: file.path,
          line: lineOf(file.content, match.index ?? 0),
          pattern: 'bare-500-in-catch',
          snippet: 'catch { ... status: 500 } with no capture',
        })
      }
    }
  }

  return hits.sort((a, b) => (a.path === b.path ? a.line - b.line : a.path.localeCompare(b.path)))
}

/**
 * Hits that are NEW in this change, given the added lines per file.
 *
 * `addedLines` is what a diff gives you: for each changed path, the line numbers that were added. A hit
 * on an added line is the author's; anything else was already there and is reported, not blocked.
 */
export function newSilentFailures(
  hits: readonly SilentFailureHit[],
  addedLines: ReadonlyMap<string, ReadonlySet<number>>,
): SilentFailureHit[] {
  return hits.filter((hit) => addedLines.get(hit.path)?.has(hit.line) ?? false)
}
