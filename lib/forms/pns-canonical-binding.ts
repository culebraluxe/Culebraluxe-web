import 'server-only'

import { randomUUID } from 'node:crypto'

import { sql } from '@/db/client'
import { SqlContractRepository } from '@/db/contract-service-repository'
import { SqlFirmRepository } from '@/db/firm-service-repository'
import { SqlPersonRepository } from '@/db/person-service-repository'
import { SqlPropertyRepository } from '@/db/property-service-repository'
import { composeCoreServices } from '@/services/composition'
import { CONTRACT_OPERATIONS, type ContractDto, type ContractRoleDto } from '@/services/contract'
import { FIRM_OPERATIONS, type FirmDto } from '@/services/firm'
import { PERSON_OPERATIONS } from '@/services/person'
import { PROPERTY_OPERATIONS, type PropertyDto } from '@/services/property'
import {
  toPnsCanonicalValue,
  toPnsWorkingValue,
  type PnsCanonicalFields,
  type PnsCanonicalSnapshot,
  type PnsFieldOrigin,
  type SavePnsCanonicalRequest,
} from './pns-canonical-types'

const TEMPLATE_ID = 'PR-PNS'
const CONTRACT_TYPE = 'purchase_sale'

const core = composeCoreServices({
  person: new SqlPersonRepository(),
  firm: new SqlFirmRepository(),
  property: new SqlPropertyRepository(),
  contract: new SqlContractRepository(),
})

type PnsFormEvidenceRow = {
  id: string
  property_id: string | null
  field_values: unknown
  updated_at: string | Date
}
type ContractIdRow = { id: string }
type FormEvidence = {
  id: string
  propertyId: string | null
  fields: Record<string, string>
  updatedAt: string
}

function compact(value: unknown): string {
  return typeof value === 'string' ? value.trim() : ''
}

function normalized(value: string | null | undefined): string {
  return compact(value).toLowerCase().replace(/\s+/g, ' ')
}

function asFieldValues(value: unknown): Record<string, string> {
  if (typeof value === 'string') {
    try {
      return asFieldValues(JSON.parse(value) as unknown)
    } catch {
      return {}
    }
  }
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {}
  const result: Record<string, string> = {}
  for (const [key, raw] of Object.entries(value)) {
    if (typeof raw === 'string') result[key] = raw
    else if (typeof raw === 'number' || typeof raw === 'boolean') result[key] = String(raw)
  }
  return result
}

function iso(value: string | Date): string {
  const date = value instanceof Date ? value : new Date(value)
  return Number.isNaN(date.getTime()) ? String(value) : date.toISOString()
}

async function serviceValue<T>(
  promise: Promise<{ ok: true; value: T } | { ok: false; error: { code: string; message: string } }>,
  label: string,
): Promise<T> {
  const result = await promise
  if (!result.ok) throw new Error(`${label}: ${result.error.code} ${result.error.message}`)
  return result.value
}

function serviceContext(actorId: string | null = null) {
  return {
    actor: { id: actorId, kind: actorId ? 'user' as const : 'system' as const },
    correlationId: randomUUID(),
  }
}

async function latestPnsEvidence(personId: string): Promise<FormEvidence | null> {
  const rows = (await sql`
    select f.id, f.property_id, f.field_values, f.updated_at
    from document_form_instance f
    left join deal d on d.id = f.deal_id
    where f.template_id = ${TEMPLATE_ID}
      and (
        f.person_id = ${personId}
        or d.client_person_id = ${personId}
        or exists (
          select 1
          from deal_participant dp
          where dp.deal_id = f.deal_id
            and dp.person_id = ${personId}
            and dp.active = true
        )
      )
    order by f.updated_at desc, f.id desc
    limit 1
  `) as PnsFormEvidenceRow[]
  const row = rows[0]
  if (!row) return null
  return {
    id: row.id,
    propertyId: row.property_id,
    fields: asFieldValues(row.field_values),
    updatedAt: iso(row.updated_at),
  }
}

async function findDraftContractId(personId: string, evidenceId: string | null): Promise<string | null> {
  const rows = (await sql`
    select distinct c.id
    from contract c
    left join contract_person cp on cp.contract_id = c.id
    left join role r on r.id = cp.role_id and r.scope = cp.role_scope
    where c.form_template_id = ${TEMPLATE_ID}
      and c.status = 'draft'
      and (
        (${evidenceId}::uuid is not null and c.source_form_instance_id = ${evidenceId}::uuid)
        or (
          cp.person_id = ${personId}
          and r.scope = 'contract_person'
          and r.code in ('SELLER', 'SELLER_REPRESENTATIVE')
        )
      )
    order by c.id desc
    limit 1
  `) as ContractIdRow[]
  return rows[0]?.id ?? null
}

function put(
  fields: Record<string, string>,
  origins: Record<string, PnsFieldOrigin>,
  name: string,
  value: string | null | undefined,
  origin: PnsFieldOrigin,
  overwrite = true,
): void {
  const next = compact(value)
  if (!next) return
  if (!overwrite && compact(fields[name])) return
  fields[name] = next
  origins[name] = origin
}

function roleField(role: ContractRoleDto): string | null {
  if (role.kind === 'person') {
    switch (role.roleCode) {
      case 'BUYER': return 'buyerName'
      case 'SELLER': return 'sellerName'
      case 'BUYER_BROKER': return 'buyerBrokerName'
      case 'SELLER_BROKER': return 'sellerBrokerName'
      case 'SELLER_SPOUSE': return 'spouseName'
      case 'CLOSING_NOTARY': return 'notaryName'
      case 'LENDER_CONTACT': return 'lenderName'
      default: return null
    }
  }
  switch (role.roleCode) {
    case 'BUYER': return 'buyerName'
    case 'SELLER': return 'sellerName'
    case 'LENDER': return 'lenderName'
    case 'ESCROW_HOLDER': return 'escrowHolder'
    default: return null
  }
}

function overlayProperty(
  fields: Record<string, string>,
  origins: Record<string, PnsFieldOrigin>,
  property: PropertyDto | null,
): void {
  if (!property) return
  put(fields, origins, 'property', property.localName ?? property.displayName, 'property')
  put(fields, origins, 'municipality', property.municipality, 'property')
  put(fields, origins, 'catastroNumber', property.catastroNumber, 'property')
  put(fields, origins, 'registryEntry', property.registryEntry, 'property')
  put(fields, origins, 'fincaNumber', property.fincaNumber, 'property')
  put(fields, origins, 'registrySection', property.registrySection, 'property')
}

export async function loadPnsCanonicalSnapshot(
  personId: string,
  requestedContractId?: string | null,
): Promise<PnsCanonicalSnapshot> {
  const cleanPersonId = personId.trim()
  if (!cleanPersonId) throw new Error('personId is required.')

  const [person, propertyContext, evidence] = await Promise.all([
    serviceValue(
      core.person.execute({
        operation: PERSON_OPERATIONS.GET,
        payload: { personId: cleanPersonId },
        context: serviceContext(),
      }),
      'Person lookup failed',
    ),
    serviceValue(
      core.property.execute({
        operation: PROPERTY_OPERATIONS.FOR_PERSON,
        payload: { personId: cleanPersonId },
        context: serviceContext(),
      }),
      'Property context failed',
    ),
    latestPnsEvidence(cleanPersonId),
  ])
  if (!person) throw new Error(`Person not found: ${cleanPersonId}`)

  const requestedId = compact(requestedContractId)
  const contractId = requestedId || await findDraftContractId(cleanPersonId, evidence?.id ?? null)
  const contract = contractId
    ? await serviceValue(
        core.contract.execute({
          operation: CONTRACT_OPERATIONS.GET,
          payload: { contractId },
          context: serviceContext(),
        }),
        'Contract lookup failed',
      )
    : null

  let physical = propertyContext.properties.find((row) => row.relation === 'physical_property')?.property ?? null
  const preferredPropertyId = contract?.propertyId ?? evidence?.propertyId ?? null
  if (preferredPropertyId && physical?.id !== preferredPropertyId) {
    const preferred = await serviceValue(
      core.property.execute({
        operation: PROPERTY_OPERATIONS.GET,
        payload: { propertyId: preferredPropertyId },
        context: serviceContext(),
      }),
      'P&S Property lookup failed',
    )
    if (preferred) physical = preferred
  }

  const fields: Record<string, string> = { ...(evidence?.fields ?? {}) }
  const origins: Record<string, PnsFieldOrigin> = Object.fromEntries(
    Object.keys(fields).map((name) => [name, 'pns_form' as const]),
  )

  overlayProperty(fields, origins, physical)
  put(fields, origins, 'sellerName', person.displayName, 'person', false)

  for (const role of contract?.roles ?? []) {
    const field = roleField(role)
    if (field) put(fields, origins, field, role.snapshotName, role.kind === 'firm' ? 'firm' : 'role', false)
  }
  for (const [name, raw] of Object.entries(contract?.facts ?? {})) {
    if (typeof raw === 'string' || typeof raw === 'number' || typeof raw === 'boolean') {
      put(fields, origins, name, String(raw), 'contract', true)
    }
  }

  return {
    personId: cleanPersonId,
    personDisplayName: person.displayName,
    contractId: contract?.id ?? null,
    contractStatus: contract?.status ?? null,
    formInstanceId: evidence?.id ?? null,
    formUpdatedAt: evidence?.updatedAt ?? null,
    physicalPropertyId: physical?.id ?? preferredPropertyId,
    fields,
    origins,
    roles: contract?.roles ?? [],
  }
}

async function findFirm(name: string): Promise<FirmDto | null> {
  const clean = toPnsCanonicalValue(name)
  if (!clean) return null
  return serviceValue(
    core.firm.execute({
      operation: FIRM_OPERATIONS.FIND_BY_NAME,
      payload: { name: clean },
      context: serviceContext(),
    }),
    `Firm lookup failed for ${clean}`,
  )
}

function keepExistingRole(role: ContractRoleDto, fields: PnsCanonicalFields): boolean {
  const field = roleField(role)
  if (!field) return true
  const current = toPnsCanonicalValue(fields[field])
  const snapshot = toPnsCanonicalValue(role.snapshotName)
  return Boolean(current && snapshot && normalized(snapshot) === normalized(current))
}

function dedupeRoles(roles: readonly ContractRoleDto[]): ContractRoleDto[] {
  const seen = new Set<string>()
  const result: ContractRoleDto[] = []
  for (const role of roles) {
    const identity = role.kind === 'person' ? role.personId : role.firmId
    const key = `${role.kind}:${identity}:${role.roleCode}:${role.ordinal ?? 0}`
    if (seen.has(key)) continue
    seen.add(key)
    result.push(role)
  }
  return result
}

function personSellerCapacity(value: string): boolean {
  return value === 'Individual' || value === 'Married' || value === 'Attorney-in-fact'
}

export async function savePnsCanonicalFields(
  request: SavePnsCanonicalRequest,
  actorId: string | null,
): Promise<PnsCanonicalSnapshot> {
  const before = await loadPnsCanonicalSnapshot(request.personId, request.contractId)

  // A working Contract draft never stores accidental blank/NaN/null-like holes.
  // One explicit TBD survives until the broker resolves it on a later pass.
  const fields = Object.fromEntries(
    Object.entries(request.fields).map(([name, value]) => [name, toPnsWorkingValue(value)]),
  ) as Record<string, string>
  const canonicalField = (name: string) => toPnsCanonicalValue(fields[name])
  const context = () => serviceContext(actorId)

  const sellerName = canonicalField('sellerName')
  const sellerCapacity = canonicalField('sellerCapacity')
  const sellerIsPerson = personSellerCapacity(sellerCapacity)

  // Do not promote an unresolved legal-party assumption into Person. A valid
  // name can remain in Contract draft facts while capacity is still TBD.
  if (sellerName && sellerIsPerson) {
    await serviceValue(
      core.person.execute({
        operation: PERSON_OPERATIONS.SET_DISPLAY_NAME,
        payload: { personId: before.personId, displayName: sellerName },
        context: context(),
      }),
      'Seller Person write-back failed',
    )
  }

  const propertyFieldNames = [
    'property',
    'municipality',
    'catastroNumber',
    'registryEntry',
    'fincaNumber',
    'registrySection',
  ] as const
  const propertyHasData = propertyFieldNames.some((name) => canonicalField(name))
  let propertyId = compact(request.physicalPropertyId) || before.physicalPropertyId || null
  if (propertyHasData || propertyId) {
    const savedProperty = await serviceValue(
      core.property.execute({
        operation: PROPERTY_OPERATIONS.UPSERT_FOR_PERSON,
        payload: {
          personId: before.personId,
          relation: 'physical_property',
          propertyId: propertyId ?? undefined,
          address: canonicalField('municipality') ? { city: canonicalField('municipality') } : undefined,
          localName: canonicalField('property') || undefined,
          catastroNumber: canonicalField('catastroNumber') || undefined,
          registryEntry: canonicalField('registryEntry') || undefined,
          fincaNumber: canonicalField('fincaNumber') || undefined,
          registrySection: canonicalField('registrySection') || undefined,
          sourceType: 'pns_form',
          sourceKey: before.formInstanceId,
        },
        context: context(),
      }),
      'P&S Property write-back failed',
    )
    propertyId = savedProperty.property.id
  }
  if (!propertyId) throw new Error('P&S Contract requires a subject Property before draft save.')

  const existingContract: ContractDto | null = before.contractId
    ? await serviceValue(
        core.contract.execute({
          operation: CONTRACT_OPERATIONS.GET,
          payload: { contractId: before.contractId },
          context: context(),
        }),
        'Existing Contract lookup failed',
      )
    : null

  const roles: ContractRoleDto[] = (existingContract?.roles ?? before.roles)
    .filter((role) => role.roleCode !== 'SELLER' && role.roleCode !== 'SELLER_REPRESENTATIVE')
    .filter((role) => keepExistingRole(role, fields))
    .map((role) => {
      const field = roleField(role)
      const snapshotName = field ? canonicalField(field) : ''
      return field && snapshotName ? { ...role, snapshotName } : role
    })

  if (sellerName) {
    if (sellerCapacity === 'Entity') {
      const sellerFirm = await serviceValue(
        core.firm.execute({
          operation: FIRM_OPERATIONS.UPSERT,
          payload: { name: sellerName, legalName: sellerName, kind: 'LEGAL_ENTITY' },
          context: context(),
        }),
        'Seller Firm write-back failed',
      )
      roles.push({ kind: 'firm', firmId: sellerFirm.id, roleCode: 'SELLER', snapshotName: sellerName })
      roles.push({
        kind: 'person',
        personId: before.personId,
        roleCode: 'SELLER_REPRESENTATIVE',
        snapshotName: before.personDisplayName,
        attributes: {
          capacity: 'Entity',
          signerTitle: canonicalField('entitySignerTitle') || null,
        },
      })
    } else if (sellerIsPerson) {
      roles.push({ kind: 'person', personId: before.personId, roleCode: 'SELLER', snapshotName: sellerName })
    }
  }

  const buyerName = canonicalField('buyerName')
  const lenderName = canonicalField('lenderName')
  const escrowHolder = canonicalField('escrowHolder')
  const [buyerFirm, lenderFirm, escrowFirm] = await Promise.all([
    findFirm(buyerName),
    findFirm(lenderName),
    findFirm(escrowHolder),
  ])
  if (buyerFirm && buyerName) {
    roles.push({ kind: 'firm', firmId: buyerFirm.id, roleCode: 'BUYER', snapshotName: buyerName })
  }
  if (lenderFirm && lenderName) {
    roles.push({ kind: 'firm', firmId: lenderFirm.id, roleCode: 'LENDER', snapshotName: lenderName })
  }
  if (escrowFirm && escrowHolder) {
    roles.push({ kind: 'firm', firmId: escrowFirm.id, roleCode: 'ESCROW_HOLDER', snapshotName: escrowHolder })
  }

  const contractId = before.contractId ?? (compact(request.contractId) || randomUUID())
  const saved = await serviceValue(
    core.contract.execute({
      operation: CONTRACT_OPERATIONS.SAVE_DRAFT,
      payload: {
        contractId,
        contractType: existingContract?.contractType ?? CONTRACT_TYPE,
        formTemplateId: TEMPLATE_ID,
        sourceFormInstanceId: before.formInstanceId,
        predecessorContractId: existingContract?.predecessorContractId ?? null,
        propertyId,
        roles: dedupeRoles(roles),
        facts: fields,
      },
      context: context(),
    }),
    'Contract draft save failed',
  )

  return loadPnsCanonicalSnapshot(before.personId, saved.id)
}
