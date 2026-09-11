// ---------------------------------------------------------------------------
// ENG-FORGE-V5-23..27 — the analyzer tool capability boundary.
//
// Five tools were researched, installed and DOCUMENTED (docs/agent/skills/*), but
// never wired into the Forge runtime. This module is the shared boundary they all
// need, so each tool's authority is declared once and recomputed per transition.
//
// The rules, taken from the five stories' acceptance criteria:
//   - A tool is either MODEL-FACING (the model can choose it) or a DETERMINISTIC
//     INSTRUMENT (Assay/QA runs it; it must never appear in a model catalog).
//   - Authority is per ROLE and per PHASE, and is RECOMPUTED on every transition.
//     A resumed session cannot retain stale permission (V5-23 #6).
//   - Excluded roles get NOTHING, not a smaller set (V5-23 #4).
//   - An unavailable tool degrades EXPLICITLY to the existing repo tools and
//     never silently broadens permission (V5-23 #8, V5-24 #4).
//   - `wired` is honest. Nothing may claim a tool ran when it did not
//     (docs/agent/skills/README.md).
//
// Pure module: no process, no network, no filesystem.
// ---------------------------------------------------------------------------

/** Forge execution positions. Mirrors the engine's node vocabulary. */
export type ForgeToolRole =
  | 'scout'
  | 'architect'
  | 'lead_pre'
  | 'lead_solo'
  | 'lead_post'
  | 'smith'
  | 'inspector'
  | 'assay'
  | 'dev_ops'

export type ForgeToolId = 'ripwire' | 'serena' | 'rtk' | 'semgrep' | 'knip' | 'cruiser'

/**
 * model-facing    — the model may choose it from its catalog.
 * deterministic   — run by Assay/verification. MUST NOT appear in a model catalog.
 * transparent     — applied by the runtime BENEATH the model (output compaction).
 *                   It is never a tool the model chooses and never a schema the
 *                   model sees, so it must not appear in a catalog either.
 */
export type ForgeToolClass = 'model-facing' | 'deterministic' | 'transparent'

/** Serena's semantic operations, split by authority. */
export type SerenaOperation =
  | 'symbol.overview'
  | 'symbol.lookup'
  | 'symbol.references'
  | 'symbol.implementations'
  | 'symbol.diagnostics'
  | 'symbol.rename'
  | 'symbol.replace-body'
  | 'symbol.insert'

export const SERENA_READ_OPERATIONS: readonly SerenaOperation[] = [
  'symbol.overview',
  'symbol.lookup',
  'symbol.references',
  'symbol.implementations',
  'symbol.diagnostics',
]

export const SERENA_WRITE_OPERATIONS: readonly SerenaOperation[] = [
  'symbol.rename',
  'symbol.replace-body',
  'symbol.insert',
]

export function isSerenaWriteOperation(operation: SerenaOperation): boolean {
  return SERENA_WRITE_OPERATIONS.includes(operation)
}

export type ForgeToolDeclaration = {
  id: ForgeToolId
  toolClass: ForgeToolClass
  purpose: string
  skillDoc: string
  /**
   * Honest wiring status. FALSE means: declared and permissioned here, but the
   * execution seam is not connected yet — so no lane may report that it ran.
   */
  wired: boolean
  /** Positions that receive the tool at all. Absence means denial. */
  roles: readonly ForgeToolRole[]
  /** Positions that may perform MUTATING operations through this tool. */
  writeRoles: readonly ForgeToolRole[]
  /** What the lane falls back to when the tool is unavailable. */
  degradeTo: string
}

// ---------------------------------------------------------------------------
// The catalog. Role lists come from each story's scope section verbatim:
//   V5-23 Serena  — Architect read-only; Lead PRE read, SOLO/POST write; Smith
//                   full bounded; Scout/Inspector/Assay/DEV_OPS get none.
//   V5-24 RTK     — Architect/Lead/Smith. Never a model tool choice.
//   V5-25 cruiser — Assay/QA deterministic instrument, never model-facing.
//   V5-26 semgrep — Assay/QA deterministic instrument, never model-facing.
//   V5-27 knip    — maintenance/hygiene.
// Wiring status corrected 2026-09-11: cruiser, semgrep and knip ARE wired —
// workflow_app/forge/forge-static-gate.ts runs them from the Assay adapter on the
// exact candidate. This catalog previously claimed wired:false for all five,
// which was simply wrong; the README said "not yet wired" and I took it at face
// value instead of reading the assay code. serena and rtk remain unwired.
// ---------------------------------------------------------------------------

export const FORGE_TOOL_CATALOG: Readonly<Record<ForgeToolId, ForgeToolDeclaration>> = {
  ripwire: {
    id: 'ripwire',
    toolClass: 'model-facing',
    purpose: 'repo intel, blast radius, test surface',
    skillDoc: 'docs/agent/skills/ripwire.md',
    wired: true,
    roles: ['scout', 'architect', 'lead_pre', 'smith', 'inspector'],
    writeRoles: [],
    degradeTo: 'workspace.fs.read + git.diff',
  },
  serena: {
    id: 'serena',
    toolClass: 'model-facing',
    purpose: 'symbol-level navigation, references, and bounded semantic edits',
    skillDoc: 'docs/agent/skills/serena.md',
    wired: false,
    // Scout stays on Ripwire; Inspector/Assay/DEV_OPS are excluded in the initial cut.
    roles: ['architect', 'lead_pre', 'lead_solo', 'lead_post', 'smith'],
    // Lead PRE is read-only. Lead SOLO/POST and Smith may mutate.
    writeRoles: ['lead_solo', 'lead_post', 'smith'],
    degradeTo: 'ripwire + generic read/search',
  },
  rtk: {
    id: 'rtk',
    toolClass: 'transparent',
    purpose: 'transparent compaction of noisy command output',
    skillDoc: 'docs/agent/skills/rtk.md',
    wired: false,
    // Transparent interception only: the model never chooses RTK, and it may
    // never mutate anything itself.
    roles: ['architect', 'lead_pre', 'lead_solo', 'lead_post', 'smith'],
    writeRoles: [],
    degradeTo: 'the raw command path (uncompacted output)',
  },
  cruiser: {
    id: 'cruiser',
    toolClass: 'deterministic',
    purpose: 'architecture boundaries and cycles as a hard gate',
    skillDoc: 'docs/agent/skills/cruiser.md',
    wired: true,
    roles: ['assay', 'inspector'],
    writeRoles: [],
    degradeTo: 'no substitute — the gate is skipped and that omission is recorded',
  },
  semgrep: {
    id: 'semgrep',
    toolClass: 'deterministic',
    purpose: 'static and dataflow security checks on the exact candidate',
    skillDoc: 'docs/agent/skills/semgrep.md',
    wired: true,
    roles: ['assay', 'inspector'],
    writeRoles: [],
    degradeTo: 'no substitute — the gate is skipped and that omission is recorded',
  },
  knip: {
    id: 'knip',
    toolClass: 'deterministic',
    purpose: 'unused files, exports and dependencies (hygiene)',
    skillDoc: 'docs/agent/skills/knip.md',
    // Wired into the static gate 2026-09-11 (runKnip). Findings are informational
    // and are not yet persisted as a durable artifact — see V5-27 acceptance.
    wired: true,
    roles: ['inspector'],
    writeRoles: [],
    degradeTo: 'no substitute — reported as not run',
  },
}

export const FORGE_TOOL_IDS = Object.keys(FORGE_TOOL_CATALOG) as ForgeToolId[]

/**
 * Tools that must never appear in a model-facing catalog: the deterministic
 * instruments AND the transparent shims. RTK in particular must not become a
 * tool the model selects (V5-24 acceptance 1 and 8).
 */
export function modelForbiddenTools(): ForgeToolId[] {
  return FORGE_TOOL_IDS.filter((id) => FORGE_TOOL_CATALOG[id].toolClass !== 'model-facing')
}

// --- Resolution -------------------------------------------------------------

export type ToolDegradation = {
  tool: ForgeToolId
  reason: 'not-wired' | 'not-declared-for-role' | 'unavailable'
  fallback: string
}

export type ForgeToolGrant = {
  tool: ForgeToolId
  toolClass: ForgeToolClass
  /** May the model choose this tool from its catalog? */
  modelFacing: boolean
  /** May this position MUTATE through the tool? */
  canWrite: boolean
  /** Semantic operations in force, for tools that have an operation split. */
  operations: readonly SerenaOperation[]
  /** False when the grant exists on paper but the seam is not connected. */
  executable: boolean
}

export type ForgeToolResolution = {
  role: ForgeToolRole
  grants: ForgeToolGrant[]
  /** Model-visible catalog. Deterministic instruments can never appear here. */
  modelCatalog: ForgeToolId[]
  /** Instruments Assay may run (deterministic, never model-facing). */
  instruments: ForgeToolId[]
  degradations: ToolDegradation[]
  /**
   * True when this resolution was computed for THIS transition. A resumed or
   * long-lived session must recompute rather than reuse — see the module header.
   */
  recomputed: true
}

/**
 * Compute the tools in force for ONE position. Pure, and deliberately stateless:
 * there is nothing to retain, so a resumed session cannot keep a stale grant
 * across a role transition (V5-23 #6).
 *
 * `available` defaults to the catalog's honest wiring status. A caller that has
 * actually connected a tool seam passes it as available; nothing else may.
 */
export function resolveForgeToolPermissions(
  role: ForgeToolRole,
  options?: { available?: readonly ForgeToolId[] },
): ForgeToolResolution {
  const available = new Set(options?.available ?? FORGE_TOOL_IDS.filter((id) => FORGE_TOOL_CATALOG[id].wired))
  const grants: ForgeToolGrant[] = []
  const degradations: ToolDegradation[] = []

  for (const id of FORGE_TOOL_IDS) {
    const declaration = FORGE_TOOL_CATALOG[id]
    if (!declaration.roles.includes(role)) {
      // Never a degradation entry for a tool this role is simply not granted.
      continue
    }
    if (!available.has(id)) {
      degradations.push({
        tool: id,
        reason: declaration.wired ? 'unavailable' : 'not-wired',
        fallback: declaration.degradeTo,
      })
      continue
    }
    const canWrite = declaration.writeRoles.includes(role)
    grants.push({
      tool: id,
      toolClass: declaration.toolClass,
      modelFacing: declaration.toolClass === 'model-facing',
      canWrite,
      operations:
        id === 'serena'
          ? canWrite
            ? [...SERENA_READ_OPERATIONS, ...SERENA_WRITE_OPERATIONS]
            : [...SERENA_READ_OPERATIONS]
          : [],
      executable: true,
    })
  }

  return {
    role,
    grants,
    modelCatalog: grants.filter((grant) => grant.modelFacing).map((grant) => grant.tool),
    instruments: grants.filter((grant) => grant.toolClass === 'deterministic').map((grant) => grant.tool),
    degradations,
    recomputed: true,
  }
}

/** The initial cut must not hand these positions a semantic tool at all. */
export const SERENA_EXCLUDED_ROLES: readonly ForgeToolRole[] = [
  'scout',
  'inspector',
  'assay',
  'dev_ops',
]

/** Would this position be offered the tool in the model catalog? */
export function toolOfferedToModel(role: ForgeToolRole, tool: ForgeToolId): boolean {
  return resolveForgeToolPermissions(role).modelCatalog.includes(tool)
}


