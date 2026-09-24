// ---------------------------------------------------------------------------
// A SOURCE FENCE AROUND THE WHOLE REPOSITORY, not one slice of it.
//
// FAILING CLOSED IS NOT THE SAME AS NOTICING. `BaseService` refuses every
// operation when no authorization port is configured, so a service built without
// one cannot leak anything — it just does nothing, quietly. Three were shipped
// that way during the Rust conversion (`legacy/workflow_app/contract-facts.ts`
// built two, `lib/agreements/crm26-consumer.ts` one) and the reason they lasted is
// that everything they touched still "worked": empty facts, skipped consumers, no
// error anybody watched.
//
// `BaseService` now says so at construction time, in the log. This says so before
// the code runs at all, over every construction site in `app/`, `lib/` and
// `legacy/` — the same shape as the DB-boundary fence that keeps `pg` inside
// ForgeDB, and for the same reason: a rule that lives only in review is a rule
// that decays.
//
// The check is deliberately syntactic and shallow: a service construction must
// pass a SECOND argument (the infrastructure). It does not try to judge whether
// that argument includes an authorization port, because that is what the runtime
// warning and the hook test cover; what it catches is the case nothing else does —
// a service built with no infrastructure at all.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readdir, readFile } from 'node:fs/promises'
import path from 'node:path'

const REPO = new URL('../../../', import.meta.url).pathname
const ROOTS = ['app', 'lib', 'legacy']

/** Every service class that extends BaseService (the abstract service whose hook this protects). */
const SERVICE_CLASSES = [
  'CalendarService',
  'CommsService',
  'ContractService',
  'FirmService',
  'FormService',
  'MediaService',
  'PersonService',
  'ProjectService',
  'PropertyService',
  'SecurityService',
  'ShowingService',
  'VaultService',
  'WbsService',
]

const SKIP = ['node_modules', '.next', 'dist', 'build', 'coverage', 'target']

async function sourceFiles(dir: string): Promise<string[]> {
  const found: string[] = []
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    if (entry.name.startsWith('.') || SKIP.includes(entry.name)) continue
    const full = path.join(dir, entry.name)
    if (entry.isDirectory()) found.push(...(await sourceFiles(full)))
    else if (/\.tsx?$/.test(entry.name)) found.push(full)
  }
  return found
}

/** The argument list of a call, with balanced parentheses and strings respected. */
function callArguments(source: string, openParen: number): string {
  let depth = 0
  let quote: string | null = null
  for (let i = openParen; i < source.length; i++) {
    const char = source[i]
    if (quote) {
      if (char === '\\') i++
      else if (char === quote) quote = null
      continue
    }
    if (char === '"' || char === "'" || char === '`') quote = char
    else if (char === '(' || char === '[' || char === '{') depth++
    else if (char === ')' || char === ']' || char === '}') {
      depth--
      if (depth === 0) return source.slice(openParen + 1, i)
    }
  }
  return source.slice(openParen + 1)
}

/** Arguments at depth 0, i.e. the ones the constructor actually receives. */
function topLevelArgumentCount(args: string): number {
  if (!args.trim()) return 0
  let depth = 0
  let quote: string | null = null
  let count = 1
  for (let i = 0; i < args.length; i++) {
    const char = args[i]
    if (quote) {
      if (char === '\\') i++
      else if (char === quote) quote = null
      continue
    }
    if (char === '"' || char === "'" || char === '`') quote = char
    else if (char === '(' || char === '[' || char === '{') depth++
    else if (char === ')' || char === ']' || char === '}') depth--
    else if (char === ',' && depth === 0) count++
  }
  return count
}

test('every service construction in the app passes its infrastructure', async () => {
  const offenders: string[] = []

  for (const root of ROOTS) {
    for (const file of await sourceFiles(path.join(REPO, root))) {
      if (/\/tests?\//.test(file) || /\.test\.tsx?$/.test(file)) continue
      const source = await readFile(file, 'utf8')
      for (const name of SERVICE_CLASSES) {
        const pattern = new RegExp(`new ${name}\\s*\\(`, 'g')
        for (const match of source.matchAll(pattern)) {
          const args = callArguments(source, match.index + match[0].length - 1)
          if (topLevelArgumentCount(args) < 2) {
            const line = source.slice(0, match.index).split('\n').length
            offenders.push(`${path.relative(REPO, file)}:${line} — new ${name}(${args.trim()})`)
          }
        }
      }
    }
  }

  assert.deepEqual(
    offenders,
    [],
    'these services are built with no infrastructure, so every operation they run fails with ' +
      'AUTHORIZATION_UNAVAILABLE and the feature silently does nothing — pass ' +
      '{ authorization: new AuthorizationService() } or compose them through composeCoreServices()',
  )
})
