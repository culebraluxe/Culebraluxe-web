import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  buildRouteAuthorityManifest,
  discoverRouteFiles,
  evaluateDetected,
  renderManifest,
  runCheck,
  scanRouteFile,
  type ExceptionDeclaration,
} from '@/scripts/route-authority-manifest'

// Every route handler declares its authority, and drift fails. These tests run
// the SAME check the CLI runs, against the real tree, so a stale committed
// manifest or a new undecided handler fails here rather than drifting silently.

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..', '..')
const CODES = new Set(['portal.read', 'listing.write', 'deal.write', 'tech.access'])

test('every route.ts under app/api appears in the manifest', () => {
  const files = discoverRouteFiles(REPO_ROOT)
  assert.ok(files.length > 0, 'expected at least one app/api/**/route.ts')

  const { entries, violations } = buildRouteAuthorityManifest(REPO_ROOT)
  const covered = new Set(entries.map((entry) => entry.path))

  for (const file of files) {
    assert.ok(covered.has(file), `no manifest entry for ${file}`)
  }
  assert.equal(
    violations.filter((violation) => violation.rule === 'no-exported-handler').length,
    0,
    'a route.ts exported no HTTP handler',
  )
})

test('the generated manifest is complete and the committed artifact is fresh', () => {
  const { entries, violations } = runCheck(REPO_ROOT)
  assert.deepEqual(
    violations.map((violation) => `${violation.rule} ${violation.path}#${violation.method}`),
    [],
  )
  assert.ok(entries.length > 0)
  assert.equal(renderManifest(entries), fs.readFileSync(path.join(REPO_ROOT, 'docs/agent/route-authority-manifest.md'), 'utf8'))
})

test('an undecided handler with no declaration is a violation', () => {
  const detected = scanRouteFile(
    `export async function GET() { return new Response('ok') }`,
    'app/api/fixture/route.ts',
    CODES,
  )
  const { violations } = evaluateDetected(detected, {})
  assert.equal(violations.length, 1)
  assert.equal(violations[0].rule, 'undecided-handler')
})

test('an UNGUARDED declaration needs a reason and the fixing story', () => {
  const detected = scanRouteFile(
    `export async function GET() { return new Response('ok') }`,
    'app/api/fixture/route.ts',
    CODES,
  )

  const missingStory = evaluateDetected(detected, {
    'app/api/fixture/route.ts#GET': { decision: 'unguarded', reason: 'no decision yet' },
  })
  assert.ok(
    missingStory.violations.some((violation) => violation.rule === 'unguarded-missing-declaration'),
  )

  const complete: Record<string, ExceptionDeclaration> = {
    'app/api/fixture/route.ts#GET': {
      decision: 'unguarded',
      reason: 'no decision yet',
      fixStory: 'SEC-ROUTE-MANIFEST-02',
    },
  }
  const declared = evaluateDetected(detected, complete)
  assert.deepEqual(declared.violations, [])
  assert.equal(declared.entries[0].decision, 'unguarded')
  assert.equal(declared.entries[0].fixStory, 'SEC-ROUTE-MANIFEST-02')
})

test('declaration/code drift fails in both directions', () => {
  const guarded = scanRouteFile(
    `import { resolvePortalAccess } from '@/lib/auth/require-portal-access'
export async function GET() { return resolvePortalAccess(adapter, 'portal.read') }`,
    'app/api/fixture/route.ts',
    CODES,
  )
  const declaredPublic = evaluateDetected(guarded, {
    'app/api/fixture/route.ts#GET': { decision: 'public', reason: 'supposedly open' },
  })
  assert.ok(
    declaredPublic.violations.some(
      (violation) => violation.rule === 'declaration-code-drift',
    ),
  )

  const unguarded = scanRouteFile(
    `export async function GET() { return new Response('ok') }`,
    'app/api/fixture/route.ts',
    CODES,
  )
  const declaredAuthority = evaluateDetected(unguarded, {
    'app/api/fixture/route.ts#GET': { decision: 'authority', authority: 'portal.read' },
  })
  assert.ok(
    declaredAuthority.violations.some(
      (violation) => violation.rule === 'declaration-code-drift',
    ),
  )
})

test('per-method authority association is pinned on a real multi-handler route', () => {
  const { entries } = buildRouteAuthorityManifest(REPO_ROOT)
  const pns = entries.filter((entry) => entry.path === 'app/api/portal/form-sidecar/pns/route.ts')
  const byMethod = new Map(pns.map((entry) => [entry.method, entry]))
  assert.equal(byMethod.get('GET')?.authority, 'portal.read')
  assert.equal(byMethod.get('POST')?.authority, 'deal.write')
})

test('the withApiHandler wrapper resolves to the wrapped handler body', () => {
  const detected = scanRouteFile(
    `import { withApiHandler } from '@/lib/error-capture-seam'
async function GETHandler() { return runAuthorized(adapter, 'tech.access', async () => new Response('ok')) }
export const GET = withApiHandler({ label: 'x', route: '/x' }, GETHandler)`,
    'app/api/fixture/route.ts',
    CODES,
  )
  assert.equal(detected.length, 1)
  assert.equal(detected[0].authority, 'tech.access')
  assert.equal(detected[0].evidence, 'runAuthorized')
})

test('the media routes are declared session, not public', () => {
  const { entries, violations } = buildRouteAuthorityManifest(REPO_ROOT)
  const media = entries.filter(
    (entry) =>
      entry.path === 'app/api/media/[id]/route.ts' ||
      entry.path === 'app/api/media/documents/[id]/route.ts',
  )
  assert.equal(media.length, 2, 'expected both media handlers in the manifest')
  for (const entry of media) {
    assert.equal(
      entry.decision,
      'session',
      `${entry.path}#${entry.method} should be session`,
    )
  }
  assert.equal(
    violations.filter((violation) => violation.rule === 'public-with-session').length,
    0,
  )
})

test('a public declaration whose handler checks a session is a named violation', () => {
  const detected = scanRouteFile(
    `import { getToken } from 'next-auth/jwt'
export async function GET(request: Request) {
  const token = await getToken({ req: request, secret: 'x' })
  return new Response(token?.sub ? 'ok' : 'no')
}`,
    'app/api/fixture/route.ts',
    CODES,
  )
  assert.equal(detected[0].sessionEvidence, 'getToken')

  const { entries, violations } = evaluateDetected(detected, {
    'app/api/fixture/route.ts#GET': { decision: 'public', reason: 'supposedly open' },
  })
  assert.equal(violations.length, 1)
  assert.equal(violations[0].rule, 'public-with-session')
  assert.equal(entries.length, 1)
  assert.equal(entries[0].decision, 'session')
})

test('each handler appears exactly once in the manifest', () => {
  const { entries } = buildRouteAuthorityManifest(REPO_ROOT)
  const keys = entries.map((entry) => `${entry.path}#${entry.method}`)
  assert.equal(new Set(keys).size, keys.length)
})
