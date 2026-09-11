import assert from 'node:assert/strict'
import test from 'node:test'

import {
  FORGE_TOOL_CATALOG,
  FORGE_TOOL_IDS,
  SERENA_EXCLUDED_ROLES,
  modelForbiddenTools,
  resolveForgeToolPermissions,
  toolOfferedToModel,
  type ForgeToolId,
  type ForgeToolRole,
} from '../forge/forge-tool-catalog'

const ALL_ROLES: ForgeToolRole[] = [
  'scout',
  'architect',
  'lead_pre',
  'lead_solo',
  'lead_post',
  'smith',
  'inspector',
  'assay',
  'dev_ops',
]

function grantFor(role: ForgeToolRole, tool: ForgeToolId, available?: readonly ForgeToolId[]) {
  return resolveForgeToolPermissions(role, { available }).grants.find((g) => g.tool === tool)
}

// --- the honesty rule (docs/agent/skills/README.md) --------------------------

test('V5-23..27: every declared tool carries an honest wiring status', () => {
  for (const id of FORGE_TOOL_IDS) {
    const declaration = FORGE_TOOL_CATALOG[id]
    assert.equal(typeof declaration.wired, 'boolean', id)
    assert.ok(declaration.skillDoc.startsWith('docs/agent/skills/'), id)
  }
  // Corrected TWICE on 2026-09-11. First: cruiser, semgrep and knip have a real
  // execution seam (the Assay static gate). Second: only semgrep is actually
  // INSTALLED, so only semgrep can run — `wired` now means runnable, not
  // "a code path exists".
  assert.equal(FORGE_TOOL_CATALOG.semgrep.wired, true)
  assert.equal(FORGE_TOOL_CATALOG.cruiser.wired, true, 'dependency-cruiser installed 2026-09-11')
  assert.equal(FORGE_TOOL_CATALOG.knip.wired, true, 'knip installed 2026-09-11')
  assert.equal(FORGE_TOOL_CATALOG.rtk.wired, true, 'rtk shim seam wired 2026-09-11')
  // Serena's MCP registration is generated but not yet APPLIED to the host, so
  // it stays false: a registration command that has not been run is not a seam.
  assert.equal(FORGE_TOOL_CATALOG.serena.wired, false)
})

test('V5-23..27: an unwired tool never becomes a grant, and its degradation is explicit', () => {
  const resolution = resolveForgeToolPermissions('smith')
  assert.equal(grantFor('smith', 'serena'), undefined)
  const degradation = resolution.degradations.find((d) => d.tool === 'serena')
  assert.equal(degradation?.reason, 'not-wired')
  assert.match(degradation!.fallback, /ripwire/)
})

test('V5-23..27: a wired tool is granted, and declares its fallback', () => {
  const resolution = resolveForgeToolPermissions('scout')
  assert.ok(resolution.modelCatalog.includes('ripwire'))
  assert.equal(FORGE_TOOL_CATALOG.ripwire.wired, true)
})

// --- V5-23 Serena: role profiles and the read/write split --------------------

test('V5-23: Architect gets Serena READ ONLY — every mutation is denied', () => {
  const grant = grantFor('architect', 'serena', ['serena'])
  assert.ok(grant)
  assert.equal(grant!.canWrite, false)
  assert.ok(grant!.operations.includes('symbol.references'))
  for (const writeOp of ['symbol.rename', 'symbol.replace-body', 'symbol.insert'] as const) {
    assert.equal(grant!.operations.includes(writeOp), false, `${writeOp} must be denied to Architect`)
  }
})

test('V5-23: Lead PRE is read-only, Lead SOLO and POST may write', () => {
  assert.equal(grantFor('lead_pre', 'serena', ['serena'])!.canWrite, false)
  assert.equal(grantFor('lead_solo', 'serena', ['serena'])!.canWrite, true)
  assert.equal(grantFor('lead_post', 'serena', ['serena'])!.canWrite, true)
})

test('V5-23: Smith gets the bounded semantic subset including edits', () => {
  const grant = grantFor('smith', 'serena', ['serena'])!
  assert.equal(grant.canWrite, true)
  assert.ok(grant.operations.includes('symbol.rename'))
  assert.ok(grant.operations.includes('symbol.overview'))
})

test('V5-23: Scout, Inspector, Assay and DEV_OPS get no Serena at all', () => {
  for (const role of SERENA_EXCLUDED_ROLES) {
    const resolution = resolveForgeToolPermissions(role, { available: ['serena'] })
    assert.equal(grantFor(role, 'serena', ['serena']), undefined, role)
    // Excluded is not "degraded to a fallback" — it is simply not offered.
    assert.equal(resolution.degradations.some((d) => d.tool === 'serena'), false, role)
  }
})

test('V5-23: permissions are recomputed per transition, so nothing is retained', () => {
  // The same role resolved twice yields an independent, freshly computed object:
  // there is no session state to go stale, which is what makes a resumed V5-21
  // session unable to carry a previous role's grant forward.
  const first = resolveForgeToolPermissions('smith', { available: ['serena'] })
  const second = resolveForgeToolPermissions('architect', { available: ['serena'] })
  assert.equal(first.recomputed, true)
  assert.notEqual(first, second)
  assert.equal(first.grants.find((g) => g.tool === 'serena')!.canWrite, true)
  assert.equal(second.grants.find((g) => g.tool === 'serena')!.canWrite, false)
})

// --- V5-24 RTK: transparent, never a tool choice, permissions unaffected -----

test('V5-24: RTK is never offered as a model tool choice', () => {
  for (const role of ALL_ROLES) {
    assert.equal(toolOfferedToModel(role, 'rtk'), false, role)
  }
  // Even when the seam IS available, it stays out of the model catalog.
  const resolution = resolveForgeToolPermissions('smith', { available: ['rtk'] })
  assert.equal(resolution.modelCatalog.includes('rtk'), false)
})

test('V5-24: RTK never mutates and never widens lane authority', () => {
  const grant = grantFor('smith', 'rtk', ['rtk'])!
  assert.equal(grant.canWrite, false)
  assert.equal(FORGE_TOOL_CATALOG.rtk.writeRoles.length, 0)
})

test('V5-24: an unavailable RTK degrades explicitly to the raw command path', () => {
  // RTK is wired now, so unavailability comes from the environment, not the
  // wiring — and it must still be stated rather than silently skipped.
  const degradation = resolveForgeToolPermissions('smith', { available: ['ripwire'] }).degradations.find(
    (d) => d.tool === 'rtk',
  )
  assert.equal(degradation?.reason, 'unavailable')
  assert.match(degradation!.fallback, /raw command path/)
})

// --- V5-25 / V5-26: deterministic instruments are never model-facing ---------

test('V5-25/V5-26: cruiser, semgrep and knip can never appear in a model catalog', () => {
  // RTK is also model-forbidden, but as a transparent shim rather than an instrument.
  assert.deepEqual(modelForbiddenTools().sort(), ['cruiser', 'knip', 'rtk', 'semgrep'])
  for (const role of ALL_ROLES) {
    for (const tool of ['cruiser', 'semgrep', 'knip'] as ForgeToolId[]) {
      assert.equal(toolOfferedToModel(role, tool), false, `${role}:${tool}`)
    }
  }
})

test('V5-25/V5-26: the instruments belong to Assay/QA and never auto-fix', () => {
  assert.deepEqual(FORGE_TOOL_CATALOG.cruiser.roles, ['assay', 'inspector'])
  assert.deepEqual(FORGE_TOOL_CATALOG.semgrep.roles, ['assay', 'inspector'])
  // No writer anywhere: dependency-cruiser / semgrep must not mutate the repo.
  for (const id of ['cruiser', 'semgrep', 'knip'] as ForgeToolId[]) {
    assert.equal(FORGE_TOOL_CATALOG[id].writeRoles.length, 0, id)
  }
})

test('V5-25/V5-26: when an instrument is unavailable the omission is recorded, not hidden', () => {
  const resolution = resolveForgeToolPermissions('assay', { available: ['cruiser'] })
  assert.ok(resolution.instruments.includes('cruiser'))
  const semgrep = resolution.degradations.find((d) => d.tool === 'semgrep')
  // Wired, but not available in this environment — a different reason than
  // 'not-wired', and both are stated rather than silently skipped.
  assert.equal(semgrep?.reason, 'unavailable')
  assert.match(semgrep!.fallback, /skipped and that omission is recorded/)
})

test('V5-25/V5-26: the installed instruments are granted by default', () => {
  const resolution = resolveForgeToolPermissions('assay')
  // Both installed 2026-09-11, so both are granted; neither is model-facing.
  assert.ok(resolution.instruments.includes('cruiser'))
  assert.ok(resolution.instruments.includes('semgrep'))
  assert.equal(resolution.modelCatalog.includes('cruiser'), false)
  assert.equal(resolution.degradations.some((d) => d.tool === 'cruiser'), false)
})

// --- V5-27 knip --------------------------------------------------------------

test('V5-27: knip is a hygiene instrument for the inspector, not the model', () => {
  assert.equal(FORGE_TOOL_CATALOG.knip.toolClass, 'deterministic')
  assert.deepEqual(FORGE_TOOL_CATALOG.knip.roles, ['inspector'])
  assert.equal(toolOfferedToModel('inspector', 'knip'), false)
  assert.equal(grantFor('inspector', 'knip', ['knip'])!.canWrite, false)
})
