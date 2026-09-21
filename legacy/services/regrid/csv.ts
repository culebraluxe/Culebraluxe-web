import { createHash } from 'node:crypto'

export type RegridCsvRawPayload = Record<string, string | null>

export type RegridCulebraCsvRow = {
  llUuid: string
  parcelNumber: string | null
  catastro: string | null
  numCatastro: string | null
  oldPid: string | null
  municipio: string | null
  physicalAddress: string | null
  mailingAddress: string | null
  owner: string | null
  buyerName: string | null
  address: string | null
  urbanization: string | null
  city: string | null
  county: string | null
  state: string | null
  zip: string | null
  latitude: number | null
  longitude: number | null
  insideX: number | null
  insideY: number | null
  gisAcres: number | null
  gisSquareFeet: number | null
  buildingFootprintSquareFeet: number | null
  buildingCount: number | null
  parcelValue: number | null
  landValue: number | null
  improvementValue: number | null
  taxableValue: number | null
  cabida: number | null
  salePrice: number | null
  saleDate: string | null
  deedNumber: string | null
  book: string | null
  page: string | null
  legalDescription: string | null
  zoning: string | null
  path: string | null
  lastRefresh: string | null
  regridUpdatedAt: string | null
  sourceRowNumber: number
  sourceRowSha256: string
  rawPayload: RegridCsvRawPayload
}

export type RegridCsvReject = {
  sourceRowNumber: number
  reason: string
}

export type RegridCulebraCsvParseResult = {
  headers: string[]
  rows: RegridCulebraCsvRow[]
  rejected: RegridCsvReject[]
  stats: {
    parsed: number
    accepted: number
    rejected: number
    withCatastro: number
    withPhysicalAddress: number
    withOwner: number
    withLatLon: number
  }
}

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

/** RFC-4180-ish parser with quoted commas/newlines and doubled quote support. */
export function parseCsvRecords(content: string): string[][] {
  const rows: string[][] = []
  let row: string[] = []
  let field = ''
  let quoted = false

  const emitRow = () => {
    row.push(field)
    if (row.some((value) => value !== '')) rows.push(row)
    row = []
    field = ''
  }

  for (let i = 0; i < content.length; i += 1) {
    const ch = content[i]
    if (quoted) {
      if (ch === '"') {
        if (content[i + 1] === '"') {
          field += '"'
          i += 1
        } else {
          quoted = false
        }
      } else {
        field += ch
      }
      continue
    }

    if (ch === '"' && field.length === 0) {
      quoted = true
    } else if (ch === ',') {
      row.push(field)
      field = ''
    } else if (ch === '\n') {
      emitRow()
    } else if (ch === '\r') {
      if (content[i + 1] === '\n') i += 1
      emitRow()
    } else {
      field += ch
    }
  }

  if (quoted) throw new Error('Malformed CSV: unterminated quoted field')
  if (field.length > 0 || row.length > 0) emitRow()
  return rows
}

function text(value: string | null | undefined): string | null {
  const result = value?.trim() ?? ''
  return result || null
}

function numberValue(value: string | null | undefined): number | null {
  const normalized = text(value)?.replace(/,/g, '')
  if (!normalized) return null
  const parsed = Number(normalized)
  return Number.isFinite(parsed) ? parsed : null
}

function integerValue(value: string | null | undefined): number | null {
  const parsed = numberValue(value)
  return parsed === null ? null : Math.trunc(parsed)
}

function normalizeZip(value: string | null | undefined): string | null {
  const normalized = text(value)
  if (!normalized) return null
  const numeric = normalized.match(/^(\d+)(?:\.0+)?$/)?.[1]
  if (numeric && numeric.length <= 5) return numeric.padStart(5, '0')
  return normalized
}

function normalizeDate(value: string | null | undefined): string | null {
  const normalized = text(value)
  if (!normalized) return null
  const match = normalized.match(/^(\d{4})[/-](\d{2})[/-](\d{2})/)
  return match ? `${match[1]}-${match[2]}-${match[3]}` : null
}

function normalizeTimestamp(value: string | null | undefined): string | null {
  const normalized = text(value)
  if (!normalized) return null
  return normalized.replace(/^(\d{4})\/(\d{2})\/(\d{2})/, '$1-$2-$3')
}

function sha256(value: string): string {
  return createHash('sha256').update(value).digest('hex')
}

function rawObject(headers: string[], cells: string[]): RegridCsvRawPayload {
  const result: RegridCsvRawPayload = {}
  headers.forEach((header, index) => {
    result[header] = cells[index] === '' ? null : cells[index]
  })
  return result
}

function fromRaw(raw: RegridCsvRawPayload, sourceRowNumber: number): RegridCulebraCsvRow {
  const llUuid = text(raw.ll_uuid)
  if (!llUuid || !UUID_RE.test(llUuid)) {
    throw new Error('missing or invalid ll_uuid')
  }

  const county = text(raw.county)
  const municipio = text(raw.municipio)
  const isCulebra = [county, municipio]
    .filter((value): value is string => Boolean(value))
    .some((value) => value.toLowerCase() === 'culebra')
  if (!isCulebra) throw new Error('row is not identified as Culebra')

  return {
    llUuid,
    parcelNumber: text(raw.parcelnumb),
    catastro: text(raw.catastro),
    numCatastro: text(raw.num_catastro),
    oldPid: text(raw.oldpid),
    municipio,
    physicalAddress: text(raw.direccion_fisica),
    mailingAddress: text(raw.direccion_postal),
    owner: text(raw.owner),
    buyerName: text(raw.buyername),
    address: text(raw.address),
    urbanization: text(raw.urbanization),
    city: text(raw.city),
    county,
    state: text(raw.state2),
    zip: normalizeZip(raw.szip),
    latitude: numberValue(raw.lat),
    longitude: numberValue(raw.lon),
    insideX: numberValue(raw.inside_x),
    insideY: numberValue(raw.inside_y),
    gisAcres: numberValue(raw.ll_gisacre),
    gisSquareFeet: integerValue(raw.ll_gissqft),
    buildingFootprintSquareFeet: integerValue(raw.ll_bldg_footprint_sqft),
    buildingCount: integerValue(raw.ll_bldg_count),
    parcelValue: numberValue(raw.parval),
    landValue: numberValue(raw.landval),
    improvementValue: numberValue(raw.improvval),
    taxableValue: numberValue(raw.taxable),
    cabida: numberValue(raw.cabida),
    salePrice: numberValue(raw.saleprice),
    saleDate: normalizeDate(raw.saledate),
    deedNumber: text(raw.deednum),
    book: text(raw.book),
    page: text(raw.page),
    legalDescription: text(raw.legaldesc),
    zoning: text(raw.zoning),
    path: text(raw.path),
    lastRefresh: normalizeDate(raw.ll_last_refresh),
    regridUpdatedAt: normalizeTimestamp(raw.ll_updated_at),
    sourceRowNumber,
    sourceRowSha256: sha256(JSON.stringify(raw)),
    rawPayload: raw,
  }
}

export function parseRegridCulebraCsv(content: string): RegridCulebraCsvParseResult {
  const records = parseCsvRecords(content)
  if (records.length === 0) throw new Error('Regrid CSV is empty')

  const headers = records[0].map((value, index) =>
    (index === 0 ? value.replace(/^\uFEFF/, '') : value).trim(),
  )
  const required = ['ll_uuid', 'catastro', 'direccion_fisica', 'county']
  const missing = required.filter((name) => !headers.includes(name))
  if (missing.length > 0) {
    throw new Error(`Regrid CSV missing required columns: ${missing.join(', ')}`)
  }

  const rows: RegridCulebraCsvRow[] = []
  const rejected: RegridCsvReject[] = []
  let parsed = 0

  for (let index = 1; index < records.length; index += 1) {
    const cells = records[index]
    if (cells.every((cell) => cell === '')) continue
    parsed += 1
    const sourceRowNumber = index + 1
    if (cells.length !== headers.length) {
      rejected.push({
        sourceRowNumber,
        reason: `column count ${cells.length} does not match header count ${headers.length}`,
      })
      continue
    }

    try {
      rows.push(fromRaw(rawObject(headers, cells), sourceRowNumber))
    } catch (error) {
      rejected.push({
        sourceRowNumber,
        reason: error instanceof Error ? error.message : String(error),
      })
    }
  }

  return {
    headers,
    rows,
    rejected,
    stats: {
      parsed,
      accepted: rows.length,
      rejected: rejected.length,
      withCatastro: rows.filter((row) => row.catastro).length,
      withPhysicalAddress: rows.filter((row) => row.physicalAddress).length,
      withOwner: rows.filter((row) => row.owner).length,
      withLatLon: rows.filter((row) => row.latitude !== null && row.longitude !== null).length,
    },
  }
}
