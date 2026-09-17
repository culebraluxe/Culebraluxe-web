#!/usr/bin/env node
// ---------------------------------------------------------------------------
// ROUTE AUTHORITY MANIFEST — every handler declares its authority, and drift fails.
//
// The unit is the HANDLER (path, method), never the file: several route.ts files
// export more than one method, and each method makes its own authority decision.
//
// A handler is decided when exactly one of these is true:
//   - a canonical guard is detected in its body
//       resolvePortalAccess | runAuthorized | guardPortalUpload | guardPortalRoute | requireAuthority
//     (withApiHandler is error capture, NOT a guard);
//   - the hand-kept ROUTE_AUTHORITY_EXCEPTIONS registry declares it public,
//     machine (signature / shared secret) or UNGUARDED with a reason and, where
//     a guard is still owed, the fixing story.
//
// Anything else is a violation: a handler that decides nothing, a declaration
// that disagrees with the code, or a committed manifest that no longer matches a
// fresh generation. The manifest is GENERATED, never hand-edited — run `--write`.
//
//   node --import tsx scripts/route-authority-manifest.ts --check
//   node --import tsx scripts/route-authority-manifest.ts --write
//   node --import tsx scripts/route-authority-manifest.ts --print
// ---------------------------------------------------------------------------

import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import ts from 'typescript'

export const HTTP_METHODS = [
  'GET',
  'POST',
  'PUT',
  'PATCH',
  'DELETE',
  'OPTIONS',
  'HEAD',
] as const

// The ONLY call expressions that count as an authority decision.
export const GUARD_FUNCTIONS = new Set([
  'resolvePortalAccess',
  'runAuthorized',
  'guardPortalUpload',
  'guardPortalRoute',
  'requireAuthority',
])

export const MANIFEST_REL_PATH = 'docs/agent/route-authority-manifest.md'
const AUTHORITY_TYPES_REL_PATH = 'lib/auth/types.ts'
const ROUTES_REL_ROOT = 'app/api'

export type AuthorityDecision = 'authority' | 'public' | 'machine' | 'unguarded'

export type RouteAuthorityEntry = {
  path: string
  method: string
  decision: AuthorityDecision
  authority: string | null
  evidence: string
  reason: string | null
  fixStory: string | null
}

export type ExceptionDeclaration = {
  decision: AuthorityDecision
  authority?: string
  reason?: string
  fixStory?: string
}

export type RouteAuthorityViolation = {
  path: string
  method: string
  rule: string
  detail: string
}

export type DetectedHandler = {
  path: string
  method: string
  authority: string | null
  evidence: string | null
}

// ---------------------------------------------------------------------------
// The hand-kept declarations for handlers that do NOT carry a Portal authority.
// These are the ONLY hand-written inputs; the manifest itself is generated.
// A declared `public`/`machine` handler that grows a guard is drift. A handler
// that is neither guarded nor declared here is a violation.
// ---------------------------------------------------------------------------
export const ROUTE_AUTHORITY_EXCEPTIONS: Record<string, ExceptionDeclaration> = {
  'app/api/auth/[...nextauth]/route.ts#GET': {
    decision: 'public',
    reason:
      'Auth.js sign-in, callback, signout and session handlers must be reachable before authentication.',
  },
  'app/api/auth/[...nextauth]/route.ts#POST': {
    decision: 'public',
    reason:
      'Auth.js sign-in, callback, signout and session handlers must be reachable before authentication.',
  },
  'app/api/build-info/route.ts#GET': {
    decision: 'public',
    reason:
      'Deploy-verification endpoint; exposes version, commit and build time only, no secret.',
  },
  'app/api/media/[id]/route.ts#GET': {
    decision: 'public',
    reason:
      'Public media gated by Property publication state; authenticated portal escape hatch is JWT-only.',
  },
  'app/api/media/documents/[id]/route.ts#GET': {
    decision: 'public',
    reason:
      'Document access decided by decideDocumentAccess against publication state and portal session.',
  },
  'app/api/portal/client-error/route.ts#POST': {
    decision: 'public',
    reason:
      'Anonymous by design: the portal error boundary reports failures even when auth or the database are down.',
  },
  'app/api/portal/move-trace/route.ts#POST': {
    decision: 'public',
    reason:
      'Anonymous by design: the board reports drag observations from the browser; stores clipped labels only.',
  },
  'app/api/system/agreement-execution-recovery/route.ts#POST': {
    decision: 'machine',
    reason:
      'Machine caller: fails closed unless AGREEMENT_EXECUTION_RECOVERY_KEY is set and x-recovery-key matches.',
  },
  'app/api/integrations/whatsapp/webhook/route.ts#GET': {
    decision: 'machine',
    reason:
      'Machine caller: Meta webhook handshake token verification (GET) and HMAC signature (POST).',
  },
  'app/api/integrations/whatsapp/webhook/route.ts#POST': {
    decision: 'machine',
    reason:
      'Machine caller: Meta webhook handshake token verification (GET) and HMAC signature (POST).',
  },
  'app/api/integrations/boldsign/webhook/route.ts#POST': {
    decision: 'machine',
    reason: 'Machine caller: BoldSign webhook signature verification.',
  },
}

// ---------------------------------------------------------------------------
// Authority codes are read from the canonical union, never duplicated here.
// ---------------------------------------------------------------------------
export function loadAuthorityCodes(rootDir: string): Set<string> {
  const source = fs.readFileSync(path.join(rootDir, AUTHORITY_TYPES_REL_PATH), 'utf8')
  const union = source.match(/export type AuthorityCode\s*=([\s\S]*?)(?:\n\n|\nexport )/)
  if (!union) {
    throw new Error(
      `route-authority-manifest: could not locate the AuthorityCode union in ${AUTHORITY_TYPES_REL_PATH}`,
    )
  }
  const codes = new Set<string>()
  for (const match of union[1].matchAll(/'([^']+)'/g)) codes.add(match[1])
  if (codes.size === 0) {
    throw new Error(
      `route-authority-manifest: AuthorityCode union in ${AUTHORITY_TYPES_REL_PATH} is empty`,
    )
  }
  return codes
}

// ---------------------------------------------------------------------------
// Filesystem discovery
// ---------------------------------------------------------------------------
function walkRouteFiles(dir: string, rootDir: string, out: string[]): void {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name)
    if (entry.isDirectory()) walkRouteFiles(full, rootDir, out)
    else if (entry.isFile() && entry.name === 'route.ts') {
      out.push(path.relative(rootDir, full).split(path.sep).join('/'))
    }
  }
}

export function discoverRouteFiles(rootDir: string): string[] {
  const out: string[] = []
  walkRouteFiles(path.join(rootDir, ROUTES_REL_ROOT), rootDir, out)
  return out.sort()
}

// ---------------------------------------------------------------------------
// Static detection
// ---------------------------------------------------------------------------
function hasExportModifier(node: ts.Node): boolean {
  const modifiers = ts.canHaveModifiers(node) ? ts.getModifiers(node) : undefined
  return Boolean(modifiers?.some((m) => m.kind === ts.SyntaxKind.ExportKeyword))
}

function stringLiteralValue(node: ts.Node): string | null {
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) {
    return node.text
  }
  return null
}

function guardCalleeName(expression: ts.Expression): string | null {
  if (ts.isIdentifier(expression)) return expression.text
  if (ts.isPropertyAccessExpression(expression)) return expression.name.text
  return null
}

function findGuard(
  node: ts.Node,
  authorityCodes: Set<string>,
): { authority: string; evidence: string } | null {
  let found: { authority: string; evidence: string } | null = null
  const visit = (current: ts.Node): void => {
    if (found) return
    if (ts.isCallExpression(current)) {
      const name = guardCalleeName(current.expression)
      if (name && GUARD_FUNCTIONS.has(name)) {
        for (const arg of current.arguments) {
          const value = stringLiteralValue(arg)
          if (value && authorityCodes.has(value)) {
            found = { authority: value, evidence: name }
            return
          }
        }
      }
    }
    ts.forEachChild(current, visit)
  }
  visit(node)
  return found
}

type HandlerRef = { method: string; nodes: ts.Node[] }

function collectHandlers(sourceFile: ts.SourceFile): HandlerRef[] {
  // Local declarations so `withApiHandler({...}, GETHandler)` resolves to the
  // real handler body rather than being read as an undecided wrapper.
  const localBodies = new Map<string, ts.Node>()
  for (const statement of sourceFile.statements) {
    if (ts.isFunctionDeclaration(statement) && statement.name) {
      localBodies.set(statement.name.text, statement)
    } else if (ts.isVariableStatement(statement)) {
      for (const declaration of statement.declarationList.declarations) {
        if (ts.isIdentifier(declaration.name) && declaration.initializer) {
          localBodies.set(declaration.name.text, declaration.initializer)
        }
      }
    }
  }

  const handlers: HandlerRef[] = []
  const seen = new Set<string>()
  const push = (method: string, nodes: ts.Node[]): void => {
    if (seen.has(method)) return
    seen.add(method)
    handlers.push({ method, nodes })
  }

  for (const statement of sourceFile.statements) {
    if (!hasExportModifier(statement)) continue

    if (
      ts.isFunctionDeclaration(statement) &&
      statement.name &&
      (HTTP_METHODS as readonly string[]).includes(statement.name.text)
    ) {
      push(statement.name.text, [statement])
      continue
    }

    if (!ts.isVariableStatement(statement)) continue

    for (const declaration of statement.declarationList.declarations) {
      if (
        ts.isIdentifier(declaration.name) &&
        (HTTP_METHODS as readonly string[]).includes(declaration.name.text) &&
        declaration.initializer
      ) {
        const nodes: ts.Node[] = [declaration.initializer]
        // `export const POST = POSTHandler` and
        // `export const GET = withApiHandler({...}, GETHandler)` both resolve to
        // the local handler body, where the guard actually lives.
        const references = ts.isIdentifier(declaration.initializer)
          ? [declaration.initializer]
          : ts.isCallExpression(declaration.initializer)
            ? declaration.initializer.arguments.filter(ts.isIdentifier)
            : []
        for (const reference of references) {
          const body = localBodies.get(reference.text)
          if (body) nodes.push(body)
        }
        push(declaration.name.text, nodes)
      } else if (ts.isObjectBindingPattern(declaration.name)) {
        // `export const { GET, POST } = handlers` — no local body to inspect.
        for (const element of declaration.name.elements) {
          if (
            ts.isIdentifier(element.name) &&
            (HTTP_METHODS as readonly string[]).includes(element.name.text)
          ) {
            push(element.name.text, [])
          }
        }
      }
    }
  }

  return handlers
}

export function scanRouteFile(
  source: string,
  filePath: string,
  authorityCodes: Set<string>,
): DetectedHandler[] {
  const sourceFile = ts.createSourceFile(
    filePath,
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TS,
  )
  return collectHandlers(sourceFile).map((handler) => {
    let guard: { authority: string; evidence: string } | null = null
    for (const node of handler.nodes) {
      guard = findGuard(node, authorityCodes)
      if (guard) break
    }
    return {
      path: filePath,
      method: handler.method,
      authority: guard?.authority ?? null,
      evidence: guard?.evidence ?? null,
    }
  })
}

// ---------------------------------------------------------------------------
// Evaluation: detected handlers + declarations -> entries + violations
// ---------------------------------------------------------------------------
export function evaluateDetected(
  detected: DetectedHandler[],
  exceptions: Record<string, ExceptionDeclaration> = ROUTE_AUTHORITY_EXCEPTIONS,
): { entries: RouteAuthorityEntry[]; violations: RouteAuthorityViolation[] } {
  const entries: RouteAuthorityEntry[] = []
  const violations: RouteAuthorityViolation[] = []

  for (const handler of detected) {
    const key = `${handler.path}#${handler.method}`
    const declaration = exceptions[key]

    if (handler.authority) {
      if (
        declaration &&
        declaration.decision !== 'authority' &&
        declaration.decision !== 'unguarded'
      ) {
        violations.push({
          path: handler.path,
          method: handler.method,
          rule: 'declaration-code-drift',
          detail: `declared ${declaration.decision} but code guards ${handler.authority} via ${handler.evidence}`,
        })
      } else if (
        declaration &&
        declaration.decision === 'authority' &&
        declaration.authority &&
        declaration.authority !== handler.authority
      ) {
        violations.push({
          path: handler.path,
          method: handler.method,
          rule: 'declaration-code-drift',
          detail: `declared authority ${declaration.authority} but code guards ${handler.authority}`,
        })
      }
      entries.push({
        path: handler.path,
        method: handler.method,
        decision: 'authority',
        authority: handler.authority,
        evidence: handler.evidence ?? 'guard',
        reason: null,
        fixStory: null,
      })
      continue
    }

    if (!declaration) {
      violations.push({
        path: handler.path,
        method: handler.method,
        rule: 'undecided-handler',
        detail:
          'handler has no detected guard and no UNGUARDED declaration with a reason and fixing story',
      })
      continue
    }

    if (declaration.decision === 'authority') {
      violations.push({
        path: handler.path,
        method: handler.method,
        rule: 'declaration-code-drift',
        detail: `declared authority ${declaration.authority ?? '?'} but no guard was detected in the handler`,
      })
      entries.push({
        path: handler.path,
        method: handler.method,
        decision: 'authority',
        authority: declaration.authority ?? null,
        evidence: 'exception-registry',
        reason: declaration.reason ?? null,
        fixStory: null,
      })
      continue
    }

    if (!declaration.reason) {
      violations.push({
        path: handler.path,
        method: handler.method,
        rule: 'missing-reason',
        detail: `a ${declaration.decision} declaration requires a reason`,
      })
    }
    if (declaration.decision === 'unguarded' && !declaration.fixStory) {
      violations.push({
        path: handler.path,
        method: handler.method,
        rule: 'unguarded-missing-declaration',
        detail: 'an UNGUARDED declaration requires a reason and the story that will fix it',
      })
    }

    entries.push({
      path: handler.path,
      method: handler.method,
      decision: declaration.decision,
      authority: declaration.authority ?? null,
      evidence: 'exception-registry',
      reason: declaration.reason ?? null,
      fixStory: declaration.fixStory ?? null,
    })
  }

  return { entries, violations }
}

// ---------------------------------------------------------------------------
// Manifest build + render
// ---------------------------------------------------------------------------
function compareEntries(a: RouteAuthorityEntry, b: RouteAuthorityEntry): number {
  if (a.path !== b.path) return a.path < b.path ? -1 : 1
  if (a.method !== b.method) return a.method < b.method ? -1 : 1
  return 0
}

function compareViolations(
  a: RouteAuthorityViolation,
  b: RouteAuthorityViolation,
): number {
  if (a.path !== b.path) return a.path < b.path ? -1 : 1
  if (a.method !== b.method) return a.method < b.method ? -1 : 1
  if (a.rule !== b.rule) return a.rule < b.rule ? -1 : 1
  return 0
}

function cell(value: string | null): string {
  if (!value) return ''
  return value.replace(/\|/g, '\\|')
}

export function renderManifest(entries: RouteAuthorityEntry[]): string {
  const lines: string[] = []
  lines.push('# Route Authority Manifest')
  lines.push('')
  lines.push(
    'GENERATED FILE — do not hand-edit. Regenerate with `node --import tsx scripts/route-authority-manifest.ts --write`.',
  )
  lines.push('')
  lines.push(
    'Every exported HTTP handler under `app/api/**/route.ts` appears exactly once. `decision` is `authority` (a canonical guard was detected in the handler), `public` (deliberately reachable without a Portal authority), `machine` (verified by signature or shared secret), or `unguarded` (no authority decision yet — reason and fixing story required).',
  )
  lines.push('')
  lines.push('| Path | Method | Decision | Authority | Evidence | Reason |')
  lines.push('| --- | --- | --- | --- | --- | --- |')
  for (const entry of [...entries].sort(compareEntries)) {
    lines.push(
      `| \`${entry.path}\` | ${entry.method} | ${entry.decision} | ${cell(
        entry.authority,
      )} | ${cell(entry.evidence)} | ${cell(entry.reason)} |`,
    )
  }
  lines.push('')
  lines.push(`Handlers: ${entries.length}`)
  lines.push('')
  return lines.join('\n')
}

export function buildRouteAuthorityManifest(
  rootDir: string,
  exceptions: Record<string, ExceptionDeclaration> = ROUTE_AUTHORITY_EXCEPTIONS,
): { entries: RouteAuthorityEntry[]; violations: RouteAuthorityViolation[] } {
  const authorityCodes = loadAuthorityCodes(rootDir)
  const files = discoverRouteFiles(rootDir)
  const entries: RouteAuthorityEntry[] = []
  const violations: RouteAuthorityViolation[] = []

  for (const relPath of files) {
    const source = fs.readFileSync(path.join(rootDir, relPath), 'utf8')
    const detected = scanRouteFile(source, relPath, authorityCodes)
    if (detected.length === 0) {
      violations.push({
        path: relPath,
        method: '*',
        rule: 'no-exported-handler',
        detail: 'route.ts exports no HTTP method handler',
      })
      continue
    }
    const result = evaluateDetected(detected, exceptions)
    entries.push(...result.entries)
    violations.push(...result.violations)
  }

  entries.sort(compareEntries)
  violations.sort(compareViolations)
  return { entries, violations }
}

function extractManifestPaths(committed: string): string[] {
  const paths: string[] = []
  for (const line of committed.split('\n')) {
    const match = line.match(/^\| `([^`]+)` \|/)
    if (match) paths.push(match[1])
  }
  return paths
}

export function runCheck(
  rootDir: string,
): { entries: RouteAuthorityEntry[]; violations: RouteAuthorityViolation[] } {
  const { entries, violations } = buildRouteAuthorityManifest(rootDir)
  const manifestPath = path.join(rootDir, MANIFEST_REL_PATH)

  if (!fs.existsSync(manifestPath)) {
    violations.push({
      path: MANIFEST_REL_PATH,
      method: '*',
      rule: 'manifest-missing',
      detail: 'the generated manifest has not been written; run --write',
    })
    violations.sort(compareViolations)
    return { entries, violations }
  }

  const committed = fs.readFileSync(manifestPath, 'utf8')
  const fresh = renderManifest(entries)
  if (committed !== fresh) {
    violations.push({
      path: MANIFEST_REL_PATH,
      method: '*',
      rule: 'manifest-drift',
      detail: 'the committed manifest differs from a fresh generation; run --write',
    })
  }

  for (const cited of extractManifestPaths(committed)) {
    if (!fs.existsSync(path.join(rootDir, cited))) {
      violations.push({
        path: cited,
        method: '*',
        rule: 'dead-path',
        detail: 'the manifest cites a path that does not exist (maps-cannot-cite-dead-paths)',
      })
    }
  }

  violations.sort(compareViolations)
  return { entries, violations }
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------
function main(): void {
  const rootDir = process.cwd()
  const args = process.argv.slice(2)

  if (args.includes('--write')) {
    const { entries, violations } = buildRouteAuthorityManifest(rootDir)
    if (violations.length > 0) {
      for (const v of violations) {
        console.error(`VIOLATION [${v.rule}] ${v.path}#${v.method} — ${v.detail}`)
      }
      process.exitCode = 1
      return
    }
    const manifestPath = path.join(rootDir, MANIFEST_REL_PATH)
    fs.mkdirSync(path.dirname(manifestPath), { recursive: true })
    fs.writeFileSync(manifestPath, renderManifest(entries), 'utf8')
    console.log(`route-authority-manifest: wrote ${MANIFEST_REL_PATH} (${entries.length} handlers)`)
    return
  }

  if (args.includes('--print')) {
    const { entries } = buildRouteAuthorityManifest(rootDir)
    process.stdout.write(renderManifest(entries))
    return
  }

  // Default and `--check`: fail on any violation, including manifest drift.
  const { entries, violations } = runCheck(rootDir)
  if (violations.length > 0) {
    for (const v of violations) {
      console.error(`VIOLATION [${v.rule}] ${v.path}#${v.method} — ${v.detail}`)
    }
    console.error(
      `route-authority-manifest: ${violations.length} violation(s) across ${entries.length} handlers`,
    )
    process.exitCode = 1
    return
  }
  console.log(`route-authority-manifest: ${entries.length} handlers, no violations`)
}

const isMain =
  process.argv[1] !== undefined &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
if (isMain) main()
