import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import type {
  AttachPersonIdentityRequest,
  FindPersonByIdentityRequest,
  PersonDto,
  PersonIdentityDto,
  PersonRepository,
  SetPersonDisplayNameRequest,
} from '@/services/person'

type PersonRow = {
  id: string
  display_name: string
  status: string
  archived_at: string | Date | null
}

type IdentityRow = {
  person_id: string
  identity_value: string
  source_system: string | null
  is_primary: boolean
}

function toIso(value: string | Date | null | undefined): string | null {
  if (value == null) return null
  const date = value instanceof Date ? value : new Date(value)
  return Number.isNaN(date.getTime()) ? String(value) : date.toISOString()
}

function toPerson(row: PersonRow): PersonDto {
  return {
    id: row.id,
    displayName: row.display_name,
    status: row.status,
    archivedAt: toIso(row.archived_at),
  }
}

function semanticPhone(value: string): string {
  const digits = value.replace(/\D/g, '')
  return digits.length === 11 && digits.startsWith('1') ? digits.slice(1) : digits
}

function normalizedIdentity(identity: PersonIdentityDto): string {
  if (identity.kind === 'phone') return semanticPhone(identity.value)
  if (identity.kind === 'email') return identity.value.trim().toLowerCase()
  return identity.value.trim()
}

/** SQL adapter for the canonical Person service. */
export class SqlPersonRepository implements PersonRepository {
  constructor(private readonly execute: QueryExecutor = sql) {}

  async get(personId: string): Promise<PersonDto | null> {
    const rows = (await this.execute`
      select id, display_name, status, archived_at
      from person
      where id = ${personId}
        and archived_at is null
      limit 1
    `) as PersonRow[]
    return rows[0] ? toPerson(rows[0]) : null
  }

  async findByIdentity(request: FindPersonByIdentityRequest): Promise<PersonDto | null> {
    const identity = request.identity
    const kind = identity.kind
    const value = normalizedIdentity(identity)
    const sourceSystem = identity.sourceSystem?.trim() || null
    const rows = (await this.execute`
      select p.id, p.display_name, p.status, p.archived_at
      from person_identity pi
      join person p on p.id = pi.person_id
      where p.archived_at is null
        and pi.identity_type = ${kind === 'external' ? 'external' : kind}
        and (
          (${kind} = 'phone' and
            (case
              when length(regexp_replace(pi.identity_value, '[^0-9]', '', 'g')) = 11
                and left(regexp_replace(pi.identity_value, '[^0-9]', '', 'g'), 1) = '1'
              then substring(regexp_replace(pi.identity_value, '[^0-9]', '', 'g') from 2)
              else regexp_replace(pi.identity_value, '[^0-9]', '', 'g')
            end) = ${value})
          or (${kind} = 'email' and lower(trim(pi.identity_value)) = ${value})
          or (${kind} = 'external' and pi.identity_value = ${value})
        )
        and (${sourceSystem}::text is null or pi.source_system = ${sourceSystem})
      order by p.id
      limit 2
    `) as PersonRow[]
    if (rows.length > 1) throw new Error(`Ambiguous Person identity: ${kind}:${value}`)
    return rows[0] ? toPerson(rows[0]) : null
  }

  async setDisplayName(request: SetPersonDisplayNameRequest): Promise<PersonDto> {
    const displayName = request.displayName.trim()
    if (!displayName) throw new Error('Person display name is required.')
    const rows = (await this.execute`
      update person
      set display_name = ${displayName}, updated_at = now()
      where id = ${request.personId}
        and archived_at is null
      returning id, display_name, status, archived_at
    `) as PersonRow[]
    if (!rows[0]) throw new Error(`Person not found: ${request.personId}`)
    return toPerson(rows[0])
  }

  async attachIdentity(request: AttachPersonIdentityRequest): Promise<PersonIdentityDto> {
    const identity = request.identity
    const kind = identity.kind
    const normalized = normalizedIdentity(identity)
    const sourceSystem = identity.sourceSystem?.trim() || null

    const existing = (await this.execute`
      select pi.person_id, pi.identity_value, pi.source_system, pi.is_primary
      from person_identity pi
      where pi.identity_type = ${kind === 'external' ? 'external' : kind}
        and (
          (${kind} = 'phone' and
            (case
              when length(regexp_replace(pi.identity_value, '[^0-9]', '', 'g')) = 11
                and left(regexp_replace(pi.identity_value, '[^0-9]', '', 'g'), 1) = '1'
              then substring(regexp_replace(pi.identity_value, '[^0-9]', '', 'g') from 2)
              else regexp_replace(pi.identity_value, '[^0-9]', '', 'g')
            end) = ${normalized})
          or (${kind} = 'email' and lower(trim(pi.identity_value)) = ${normalized})
          or (${kind} = 'external' and pi.identity_value = ${normalized})
        )
        and (${sourceSystem}::text is null or pi.source_system = ${sourceSystem})
      limit 2
    `) as IdentityRow[]

    if (existing.some((row) => row.person_id !== request.personId)) {
      throw new Error(`Identity already belongs to another Person: ${kind}:${normalized}`)
    }
    if (existing[0]) {
      return {
        kind,
        value: existing[0].identity_value,
        sourceSystem: existing[0].source_system ?? undefined,
        isPrimary: existing[0].is_primary,
      }
    }

    const rows = (await this.execute`
      insert into person_identity (
        person_id, identity_type, identity_value, source_system, is_primary
      ) values (
        ${request.personId},
        ${kind === 'external' ? 'external' : kind},
        ${normalized},
        ${sourceSystem},
        ${identity.isPrimary}
      )
      returning person_id, identity_value, source_system, is_primary
    `) as IdentityRow[]
    if (!rows[0]) throw new Error('Person identity insert returned no row.')
    return {
      kind,
      value: rows[0].identity_value,
      sourceSystem: rows[0].source_system ?? undefined,
      isPrimary: rows[0].is_primary,
    }
  }
}
