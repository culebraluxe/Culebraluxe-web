import assert from 'node:assert/strict'
import test from 'node:test'
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'

// ---------------------------------------------------------------------------
// A "use server" FILE MAY ONLY EXPORT ASYNC FUNCTIONS.
//
// This is a build-time rule that Next enforces at REQUEST time, and its failure mode is brutal:
// `app/portal/storyboard/actions.ts` exported an array (`RUN_RESULT_STATUSES`), the build passed, and
// from then on EVERY server action in the application failed with a generic "An error occurred in the
// Server Components render" (React #441, message stripped, no digest readable from the browser).
//
// The symptom was that the Cockpit's drags moved cards on screen and wrote NOTHING to Neon, with an
// empty `app_error` table and no failing test anywhere - hours of work chasing the board's event
// plumbing while the real cause was one exported array in a neighbouring module. So it gets a test.
// ---------------------------------------------------------------------------

const ROOTS = ['app', 'lib', 'components']

function sourceFiles(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    if (name === 'node_modules' || name === '.next' || name.startsWith('.')) continue
    const path = join(dir, name)
    const stat = statSync(path)
    if (stat.isDirectory()) {
      sourceFiles(path, out)
      continue
    }
    if (name.endsWith('.ts') || name.endsWith('.tsx')) out.push(path)
  }
  return out
}

/** True when the file has the `"use server"` directive (not merely a comment mentioning it). */
function isUseServerFile(source: string): boolean {
  const head = source.slice(0, 500)
  return /^\s*(?:\/\/[^\n]*\n|\/\*[\s\S]*?\*\/\s*)*\s*['"]use server['"]\s*(?:;|\n|$)/.test(head)
}

/** Exports whose right-hand side is not a function or a call returning one. */
function nonFunctionExports(source: string): string[] {
  const offenders: string[] = []
  const re = /^export\s+(?:const|let|var)\s+([A-Za-z0-9_$]+)\s*(?::[^=]*)?=\s*([\s\S]{0,40})/gm
  let match: RegExpExecArray | null
  while ((match = re.exec(source)) !== null) {
    const name = match[1]
    const rhs = match[2].trim()
    const looksLikeFunction = rhs.startsWith('async') || /^[A-Za-z0-9_$.]+\s*\(/.test(rhs)
    if (!looksLikeFunction) offenders.push(`${name} = ${rhs.replace(/\s+/g, ' ').trim()}`)
  }
  return offenders
}

test('no "use server" file exports anything but async functions', () => {
  const offenders: string[] = []
  for (const root of ROOTS) {
    for (const file of sourceFiles(root)) {
      const source = readFileSync(file, 'utf8')
      if (!isUseServerFile(source)) continue
      for (const bad of nonFunctionExports(source)) offenders.push(`${file}: ${bad}`)
    }
  }
  assert.deepEqual(
    offenders,
    [],
    `A "use server" module may only export async functions. Every action in the app fails at request ` +
      `time (React #441) when this is violated:\n  ${offenders.join('\n  ')}`,
  )
})
