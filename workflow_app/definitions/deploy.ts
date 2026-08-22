import { sql } from '../../db/client'
import type { ProcessGraph } from '../../workflow_engine/lib/workflow/types'
import type { ParsedProcessDefinition } from '../xml'
import { classifyDeploy, IMMUTABLE_DEFINITION_ERROR } from './version-policy'

// ---------------------------------------------------------------------------
// Generic process-definition deployment service (workflow_app owns it).
//
// Canonical path (Story 127/133):
//
//   XML file -> parse -> validate -> ProcessGraph -> upsertProcessDefinition
//
// Version/immutability contract (Story 133, ENG-12):
//   - deployed versions are immutable historical definitions
//   - running instances remain pinned to their definition_id/version
//   - changing XML means deploying a NEW version under the same logical key
//   - a version that already has instances is NEVER replaced in place
//   - duplicate redeploys are idempotent only while the version has no
//     instances (draft iteration); the explicit decision table lives in
//     version-policy.classifyDeploy and is tested without a database
//
// The table's UNIQUE (tenant_id, key, version) treats NULL tenant_id as
// distinct, so a single ON CONFLICT upsert would not dedupe tenant-less
// definitions — hence the deterministic select-then-insert/update. Deployment
// is a one-time, manual, versioned action; it is versioned and idempotent on
// repeat only while the version has no instances.
// ---------------------------------------------------------------------------

export type DeployDefinitionInput = {
  key: string
  version: number
  name: string
  description?: string | null
  graph: ProcessGraph
  createdBy?: string | null
}

export type DeployDefinitionResult = {
  id: string
  created: boolean
}

export async function upsertProcessDefinition(
  input: DeployDefinitionInput,
): Promise<DeployDefinitionResult> {
  const existing = await sql`
    select id, definition
    from process_definitions
    where tenant_id is null and key = ${input.key} and version = ${input.version}
    limit 1
  `

  if (existing[0]) {
    const id = (existing[0] as { id: string }).id
    const previousGraph = (existing[0] as { definition: ProcessGraph }).definition
    const used = await sql`
      select count(*)::int as cnt
      from process_instances
      where definition_id = ${id}
      limit 1
    `
    // ENG-12 — the explicit decision table (version-policy.classifyDeploy) is
    // the single source of truth: insert (new) / update (replaceable draft or
    // duplicate redeploy) / reject (immutable — a version that already has
    // instances is NEVER written again, even with byte-identical content).
    const decision = classifyDeploy(
      true,
      (used[0] as { cnt: number }).cnt,
      previousGraph,
      input.graph,
    )
    if (decision.action === 'reject') {
      throw new Error(
        `${decision.message} (definition '${input.key}' v${input.version})`,
      )
    }
    await sql`
      update process_definitions
      set name = ${input.name},
          description = ${input.description ?? null},
          definition = ${JSON.stringify(input.graph)}::jsonb,
          status = 'active',
          updated_at = now()
      where id = ${id}
    `
    return { id, created: false }
  }

  const rows = await sql`
    insert into process_definitions (
      tenant_id, key, version, name, description, definition, status, created_by
    ) values (
      null, ${input.key}, ${input.version}, ${input.name}, ${input.description ?? null},
      ${JSON.stringify(input.graph)}::jsonb, 'active', ${input.createdBy ?? null}
    )
    returning id
  `
  return { id: (rows[0] as { id: string }).id, created: true }
}

// ---------------------------------------------------------------------------
// Generic XML deployment path (Story 127). This is the ONLY production
// definition deployer — definitions come from XML, never hand-authored TS.
//
//   node_modules/.bin/tsx workflow_app/scripts/deploy-process-definition.ts \
//     workflow_app/definitions/RE_supermodel-v1.xml
// ---------------------------------------------------------------------------

export async function deployParsedDefinition(
  parsed: ParsedProcessDefinition,
  opts: { createdBy?: string | null } = {},
): Promise<DeployDefinitionResult> {
  return upsertProcessDefinition({
    key: parsed.key,
    version: parsed.version,
    name: parsed.name,
    description: parsed.description,
    graph: parsed.graph,
    createdBy: opts.createdBy ?? null,
  })
}
