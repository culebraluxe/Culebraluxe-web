import { db, DbFailureError, sql } from './client'
import type { QueryExecutor } from './query-executor'
import type {
  ContractDto,
  ContractEffectiveStateDto,
  ContractFirmRoleDto,
  ContractPersonRoleDto,
  ContractRepository,
  ContractRoleDto,
  CreateContractFromFormRequest,
  SaveContractDraftRequest,
  ExecuteContractRequest,
} from '@/services/contract'
import type { RoleScope } from '@/services/core'

type ContractRow = {
  id: string
  contract_type: string
  form_template_id: string
  source_form_instance_id: string | null
  predecessor_contract_id: string | null
  facts: unknown
  status: string
  executed_at: string | Date | null
  evidence_document_id: string | null
}

type ContractPropertyRow = { property_id: string }

type ContractPersonRoleRow = {
  person_id: string
  role_code: string
  ordinal: number
  snapshot_name: string | null
  attributes: unknown
}

type ContractFirmRoleRow = {
  firm_id: string
  role_code: string
  ordinal: number
  snapshot_name: string | null
  attributes: unknown
}

type ContractChainRow = {
  id: string
  facts: unknown
  depth: number
}

type IdRow = { id: string }
type ContractStatusRow = { id: string; status: string }

type TransactionRunner = <T>(
  operation: string,
  work: (tx: QueryExecutor) => Promise<T>,
) => Promise<T>

function toIso(value: string | Date | null | undefined): string | null {
  if (value == null) return null
  const date = value instanceof Date ? value : new Date(value)
  return Number.isNaN(date.getTime()) ? String(value) : date.toISOString()
}

function asRecord(value: unknown): Record<string, unknown> {
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    return { ...(value as Record<string, unknown>) }
  }
  if (typeof value === 'string') {
    try {
      const parsed = JSON.parse(value) as unknown
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        return { ...(parsed as Record<string, unknown>) }
      }
    } catch {
      // Fall through to an empty record; malformed JSON cannot be business truth.
    }
  }
  return {}
}

function canonicalRoleCode(value: string): string {
  const code = value.trim().toUpperCase()
  if (!code) throw new Error('Contract Role code is required.')
  return code
}

function ordinal(value: number | undefined): number {
  const next = value ?? 0
  if (!Number.isInteger(next) || next < 0) throw new Error(`Invalid Contract Role ordinal: ${next}`)
  return next
}

async function defaultTransaction<T>(
  operation: string,
  work: (tx: QueryExecutor) => Promise<T>,
): Promise<T> {
  const result = await db.transaction(operation, work)
  if (!result.ok) throw new DbFailureError(result.error)
  return result.data
}

async function requireRoleId(
  query: QueryExecutor,
  scope: RoleScope,
  roleCode: string,
): Promise<string> {
  const code = canonicalRoleCode(roleCode)
  const rows = (await query`
    select id
    from role
    where scope = ${scope}
      and code = ${code}
      and active = true
    limit 1
  `) as IdRow[]
  if (!rows[0]) throw new Error(`Unknown active Role ${scope}:${code}`)
  return rows[0].id
}

async function loadContract(
  query: QueryExecutor,
  contractId: string,
): Promise<ContractDto | null> {
  const contractRows = (await query`
    select
      id, contract_type, form_template_id, source_form_instance_id,
      predecessor_contract_id, facts, status, executed_at, evidence_document_id
    from contract
    where id = ${contractId}
    limit 1
  `) as ContractRow[]
  const row = contractRows[0]
  if (!row) return null

  const propertyRows = (await query`
    select cp.property_id
    from contract_property cp
    join role r
      on r.id = cp.role_id
     and r.scope = cp.role_scope
    where cp.contract_id = ${contractId}
      and r.scope = 'contract_property'
      and r.code = 'SUBJECT_PROPERTY'
    order by cp.ordinal, cp.id
    limit 1
  `) as ContractPropertyRow[]
  if (!propertyRows[0]) {
    throw new Error(`Contract ${contractId} has no SUBJECT_PROPERTY Role mapping.`)
  }

  const personRows = (await query`
    select
      cp.person_id,
      r.code as role_code,
      cp.ordinal,
      cp.snapshot_name,
      cp.attributes
    from contract_person cp
    join role r
      on r.id = cp.role_id
     and r.scope = cp.role_scope
    where cp.contract_id = ${contractId}
    order by r.code, cp.ordinal, cp.id
  `) as ContractPersonRoleRow[]

  const firmRows = (await query`
    select
      cf.firm_id,
      r.code as role_code,
      cf.ordinal,
      cf.snapshot_name,
      cf.attributes
    from contract_firm cf
    join role r
      on r.id = cf.role_id
     and r.scope = cf.role_scope
    where cf.contract_id = ${contractId}
    order by r.code, cf.ordinal, cf.id
  `) as ContractFirmRoleRow[]

  const personRoles: ContractPersonRoleDto[] = personRows.map((role) => ({
    kind: 'person',
    personId: role.person_id,
    roleCode: role.role_code,
    ordinal: role.ordinal,
    snapshotName: role.snapshot_name,
    attributes: asRecord(role.attributes),
  }))
  const firmRoles: ContractFirmRoleDto[] = firmRows.map((role) => ({
    kind: 'firm',
    firmId: role.firm_id,
    roleCode: role.role_code,
    ordinal: role.ordinal,
    snapshotName: role.snapshot_name,
    attributes: asRecord(role.attributes),
  }))
  const roles: ContractRoleDto[] = [...personRoles, ...firmRoles]

  return {
    id: row.id,
    contractType: row.contract_type,
    formTemplateId: row.form_template_id,
    sourceFormInstanceId: row.source_form_instance_id,
    predecessorContractId: row.predecessor_contract_id,
    propertyId: propertyRows[0].property_id,
    roles,
    facts: asRecord(row.facts),
    status: row.status,
    executedAt: toIso(row.executed_at),
    evidenceDocumentId: row.evidence_document_id,
  }
}

async function replaceContractMappings(
  tx: QueryExecutor,
  request: SaveContractDraftRequest,
): Promise<void> {
  await tx`delete from contract_property where contract_id = ${request.contractId}`
  await tx`delete from contract_person where contract_id = ${request.contractId}`
  await tx`delete from contract_firm where contract_id = ${request.contractId}`

  const subjectRoleId = await requireRoleId(tx, 'contract_property', 'SUBJECT_PROPERTY')
  await tx`
    insert into contract_property (
      contract_id, property_id, role_id, role_scope, ordinal
    ) values (
      ${request.contractId}, ${request.propertyId}, ${subjectRoleId}, 'contract_property', 0
    )
  `

  for (const role of request.roles) {
    const roleOrdinal = ordinal(role.ordinal)
    const attributes = JSON.stringify(role.attributes ?? {})
    if (role.kind === 'person') {
      const roleId = await requireRoleId(tx, 'contract_person', role.roleCode)
      await tx`
        insert into contract_person (
          contract_id, person_id, role_id, role_scope,
          ordinal, snapshot_name, attributes
        ) values (
          ${request.contractId}, ${role.personId}, ${roleId}, 'contract_person',
          ${roleOrdinal}, ${role.snapshotName ?? null}, ${attributes}::jsonb
        )
      `
    } else {
      const roleId = await requireRoleId(tx, 'contract_firm', role.roleCode)
      await tx`
        insert into contract_firm (
          contract_id, firm_id, role_id, role_scope,
          ordinal, snapshot_name, attributes
        ) values (
          ${request.contractId}, ${role.firmId}, ${roleId}, 'contract_firm',
          ${roleOrdinal}, ${role.snapshotName ?? null}, ${attributes}::jsonb
        )
      `
    }
  }
}

/** SQL adapter for canonical Contract persistence. */
export class SqlContractRepository implements ContractRepository {
  constructor(
    private readonly query: QueryExecutor = sql,
    private readonly transaction: TransactionRunner = defaultTransaction,
  ) {}

  async get(contractId: string): Promise<ContractDto | null> {
    return loadContract(this.query, contractId)
  }

  async createFromForm(request: CreateContractFromFormRequest): Promise<ContractDto> {
    return this.transaction('contract.createFromForm', async (tx) => {
      const existing = (await tx`
        select id from contract where id = ${request.contractId} limit 1
      `) as IdRow[]
      if (existing[0]) throw new Error(`Contract already exists: ${request.contractId}`)

      await tx`
        insert into contract (
          id, contract_type, form_template_id, source_form_instance_id,
          predecessor_contract_id, facts, status
        ) values (
          ${request.contractId},
          ${request.contractType.trim()},
          ${request.formTemplateId.trim()},
          ${request.sourceFormInstanceId ?? null}::uuid,
          ${request.predecessorContractId ?? null}::uuid,
          ${JSON.stringify(request.facts)}::jsonb,
          'draft'
        )
      `

      await replaceContractMappings(tx, request)

      const created = await loadContract(tx, request.contractId)
      if (!created) throw new Error(`Contract creation returned no row: ${request.contractId}`)
      return created
    })
  }

  async saveDraft(request: SaveContractDraftRequest): Promise<ContractDto> {
    return this.transaction('contract.saveDraft', async (tx) => {
      const rows = (await tx`
        select id, status
        from contract
        where id = ${request.contractId}
        for update
      `) as ContractStatusRow[]
      const existing = rows[0]

      if (existing && existing.status !== 'draft') {
        throw new Error(`Contract ${request.contractId} is ${existing.status}; only draft Contracts may be replaced.`)
      }

      if (existing) {
        await tx`
          update contract
          set contract_type = ${request.contractType.trim()},
              form_template_id = ${request.formTemplateId.trim()},
              source_form_instance_id = ${request.sourceFormInstanceId ?? null}::uuid,
              predecessor_contract_id = ${request.predecessorContractId ?? null}::uuid,
              facts = ${JSON.stringify(request.facts)}::jsonb,
              updated_at = now()
          where id = ${request.contractId}
        `
      } else {
        await tx`
          insert into contract (
            id, contract_type, form_template_id, source_form_instance_id,
            predecessor_contract_id, facts, status
          ) values (
            ${request.contractId},
            ${request.contractType.trim()},
            ${request.formTemplateId.trim()},
            ${request.sourceFormInstanceId ?? null}::uuid,
            ${request.predecessorContractId ?? null}::uuid,
            ${JSON.stringify(request.facts)}::jsonb,
            'draft'
          )
        `
      }

      await replaceContractMappings(tx, request)
      const saved = await loadContract(tx, request.contractId)
      if (!saved) throw new Error(`Contract draft save returned no row: ${request.contractId}`)
      return saved
    })
  }

  async getEffectiveState(contractId: string): Promise<ContractEffectiveStateDto | null> {
    const rows = (await this.query`
      with recursive chain as (
        select
          c.id,
          c.predecessor_contract_id,
          c.facts,
          0::int as depth,
          array[c.id]::uuid[] as path
        from contract c
        where c.id = ${contractId}

        union all

        select
          parent.id,
          parent.predecessor_contract_id,
          parent.facts,
          child.depth + 1,
          child.path || parent.id
        from contract parent
        join chain child on child.predecessor_contract_id = parent.id
        where child.depth < 100
          and not parent.id = any(child.path)
      )
      select id, facts, depth
      from chain
      order by depth desc
    `) as ContractChainRow[]

    if (rows.length === 0) return null
    const facts: Record<string, unknown> = {}
    const sourceContractIds: string[] = []
    for (const row of rows) {
      Object.assign(facts, asRecord(row.facts))
      sourceContractIds.push(row.id)
    }
    return { contractId, facts, sourceContractIds }
  }

  async execute(request: ExecuteContractRequest): Promise<ContractDto> {
    const rows = (await this.query`
      update contract
      set status = 'executed',
          executed_at = coalesce(executed_at, now()),
          evidence_document_id = coalesce(${request.evidenceDocumentId ?? null}::uuid, evidence_document_id),
          updated_at = now()
      where id = ${request.contractId}
      returning id
    `) as IdRow[]
    if (!rows[0]) throw new Error(`Contract not found: ${request.contractId}`)

    const contract = await this.get(request.contractId)
    if (!contract) throw new Error(`Contract disappeared after execution: ${request.contractId}`)
    return contract
  }
}
