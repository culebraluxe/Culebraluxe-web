import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import type {
  FindFirmByNameRequest,
  FirmDto,
  FirmRepository,
  UpsertFirmRequest,
} from '@/services/firm'

type FirmRow = {
  id: string
  name: string
  legal_name: string | null
  kind: string | null
  status: string
}

function compact(value: string | null | undefined): string | null {
  const next = value?.trim()
  return next ? next : null
}

function toFirm(row: FirmRow): FirmDto {
  return {
    id: row.id,
    name: row.name,
    legalName: compact(row.legal_name),
    kind: compact(row.kind),
    status: row.status,
  }
}

/** SQL adapter behind FirmService. */
export class SqlFirmRepository implements FirmRepository {
  constructor(private readonly execute: QueryExecutor = sql) {}

  async get(firmId: string): Promise<FirmDto | null> {
    const rows = (await this.execute`
      select id, name, legal_name, kind, status
      from firm
      where id = ${firmId}
      limit 1
    `) as FirmRow[]
    return rows[0] ? toFirm(rows[0]) : null
  }

  async findByName(request: FindFirmByNameRequest): Promise<FirmDto | null> {
    const name = request.name.trim()
    if (!name) return null
    const rows = (await this.execute`
      select id, name, legal_name, kind, status
      from firm
      where lower(trim(name)) = lower(trim(${name}))
         or lower(trim(coalesce(legal_name, ''))) = lower(trim(${name}))
      order by case when lower(trim(coalesce(legal_name, ''))) = lower(trim(${name})) then 0 else 1 end,
               id
      limit 2
    `) as FirmRow[]
    if (rows.length > 1 && rows[0].id !== rows[1].id) {
      throw new Error(`Ambiguous Firm name: ${name}`)
    }
    return rows[0] ? toFirm(rows[0]) : null
  }

  async upsert(request: UpsertFirmRequest): Promise<FirmDto> {
    const name = request.name.trim()
    if (!name) throw new Error('Firm name is required.')

    const existing = request.firmId
      ? await this.get(request.firmId)
      : await this.findByName({ name })

    const legalName = request.legalName === undefined
      ? existing?.legalName ?? null
      : compact(request.legalName)
    const kind = request.kind === undefined ? existing?.kind ?? null : compact(request.kind)
    const status = request.status?.trim() || existing?.status || 'active'

    if (existing) {
      const rows = (await this.execute`
        update firm
        set name = ${name},
            legal_name = ${legalName},
            kind = ${kind},
            status = ${status},
            updated_at = now()
        where id = ${existing.id}
        returning id, name, legal_name, kind, status
      `) as FirmRow[]
      if (!rows[0]) throw new Error(`Firm not found: ${existing.id}`)
      return toFirm(rows[0])
    }

    const rows = (await this.execute`
      insert into firm (name, legal_name, kind, status)
      values (${name}, ${legalName}, ${kind}, ${status})
      returning id, name, legal_name, kind, status
    `) as FirmRow[]
    if (!rows[0]) throw new Error('Firm creation returned no row.')
    return toFirm(rows[0])
  }
}
