import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import type { RegridCulebraCsvRow } from '../services/regrid/csv'

export type RegridCulebraLoadCounts = {
  accepted: number
  changed: number
  replayed: number
}

export type RegridCulebraLoadInput = {
  sourceFileSha256: string
  rows: RegridCulebraCsvRow[]
}

const BATCH_SIZE = 200

function dbPayload(row: RegridCulebraCsvRow, sourceFileSha256: string) {
  return {
    ll_uuid: row.llUuid,
    parcelnumb: row.parcelNumber,
    catastro: row.catastro,
    num_catastro: row.numCatastro,
    oldpid: row.oldPid,
    municipio: row.municipio,
    direccion_fisica: row.physicalAddress,
    direccion_postal: row.mailingAddress,
    owner: row.owner,
    buyername: row.buyerName,
    address: row.address,
    urbanization: row.urbanization,
    city: row.city,
    county: row.county,
    state2: row.state,
    szip: row.zip,
    lat: row.latitude,
    lon: row.longitude,
    inside_x: row.insideX,
    inside_y: row.insideY,
    ll_gisacre: row.gisAcres,
    ll_gissqft: row.gisSquareFeet,
    ll_bldg_footprint_sqft: row.buildingFootprintSquareFeet,
    ll_bldg_count: row.buildingCount,
    parval: row.parcelValue,
    landval: row.landValue,
    improvval: row.improvementValue,
    taxable: row.taxableValue,
    cabida: row.cabida,
    saleprice: row.salePrice,
    saledate: row.saleDate,
    deednum: row.deedNumber,
    book: row.book,
    page: row.page,
    legaldesc: row.legalDescription,
    zoning: row.zoning,
    path: row.path,
    ll_last_refresh: row.lastRefresh,
    ll_updated_at: row.regridUpdatedAt,
    source_file_sha256: sourceFileSha256,
    source_row_sha256: row.sourceRowSha256,
    source_row_number: row.sourceRowNumber,
    raw_payload: row.rawPayload,
  }
}

/** Replay-safe batch upsert for a complete/sample Culebra Regrid CSV export. */
export async function upsertRegridCulebraParcels(
  input: RegridCulebraLoadInput,
  execute: QueryExecutor = sql,
): Promise<RegridCulebraLoadCounts> {
  let changed = 0

  for (let offset = 0; offset < input.rows.length; offset += BATCH_SIZE) {
    const batch = input.rows.slice(offset, offset + BATCH_SIZE)
    const payload = JSON.stringify(batch.map((row) => dbPayload(row, input.sourceFileSha256)))

    const result = (await execute`
      with incoming as (
        select
          (item->>'ll_uuid')::uuid as ll_uuid,
          item->>'parcelnumb' as parcelnumb,
          item->>'catastro' as catastro,
          item->>'num_catastro' as num_catastro,
          item->>'oldpid' as oldpid,
          item->>'municipio' as municipio,
          item->>'direccion_fisica' as direccion_fisica,
          item->>'direccion_postal' as direccion_postal,
          item->>'owner' as owner,
          item->>'buyername' as buyername,
          item->>'address' as address,
          item->>'urbanization' as urbanization,
          item->>'city' as city,
          item->>'county' as county,
          item->>'state2' as state2,
          item->>'szip' as szip,
          (item->>'lat')::numeric as lat,
          (item->>'lon')::numeric as lon,
          (item->>'inside_x')::numeric as inside_x,
          (item->>'inside_y')::numeric as inside_y,
          (item->>'ll_gisacre')::numeric as ll_gisacre,
          (item->>'ll_gissqft')::bigint as ll_gissqft,
          (item->>'ll_bldg_footprint_sqft')::bigint as ll_bldg_footprint_sqft,
          (item->>'ll_bldg_count')::integer as ll_bldg_count,
          (item->>'parval')::numeric as parval,
          (item->>'landval')::numeric as landval,
          (item->>'improvval')::numeric as improvval,
          (item->>'taxable')::numeric as taxable,
          (item->>'cabida')::numeric as cabida,
          (item->>'saleprice')::numeric as saleprice,
          (item->>'saledate')::date as saledate,
          item->>'deednum' as deednum,
          item->>'book' as book,
          item->>'page' as page,
          item->>'legaldesc' as legaldesc,
          item->>'zoning' as zoning,
          item->>'path' as path,
          (item->>'ll_last_refresh')::date as ll_last_refresh,
          (item->>'ll_updated_at')::timestamptz as ll_updated_at,
          item->>'source_file_sha256' as source_file_sha256,
          item->>'source_row_sha256' as source_row_sha256,
          (item->>'source_row_number')::integer as source_row_number,
          coalesce(item->'raw_payload', '{}'::jsonb) as raw_payload
        from jsonb_array_elements(${payload}::jsonb) as item
      ), upserted as (
        insert into regrid_culebra_parcel as current (
          ll_uuid, parcelnumb, catastro, num_catastro, oldpid, municipio,
          direccion_fisica, direccion_postal, owner, buyername, address,
          urbanization, city, county, state2, szip, lat, lon, inside_x, inside_y,
          ll_gisacre, ll_gissqft, ll_bldg_footprint_sqft, ll_bldg_count,
          parval, landval, improvval, taxable, cabida, saleprice, saledate,
          deednum, book, page, legaldesc, zoning, path, ll_last_refresh,
          ll_updated_at, source_file_sha256, source_row_sha256, source_row_number,
          raw_payload
        )
        select
          ll_uuid, parcelnumb, catastro, num_catastro, oldpid, municipio,
          direccion_fisica, direccion_postal, owner, buyername, address,
          urbanization, city, county, state2, szip, lat, lon, inside_x, inside_y,
          ll_gisacre, ll_gissqft, ll_bldg_footprint_sqft, ll_bldg_count,
          parval, landval, improvval, taxable, cabida, saleprice, saledate,
          deednum, book, page, legaldesc, zoning, path, ll_last_refresh,
          ll_updated_at, source_file_sha256, source_row_sha256, source_row_number,
          raw_payload
        from incoming
        on conflict (ll_uuid) do update set
          parcelnumb = excluded.parcelnumb,
          catastro = excluded.catastro,
          num_catastro = excluded.num_catastro,
          oldpid = excluded.oldpid,
          municipio = excluded.municipio,
          direccion_fisica = excluded.direccion_fisica,
          direccion_postal = excluded.direccion_postal,
          owner = excluded.owner,
          buyername = excluded.buyername,
          address = excluded.address,
          urbanization = excluded.urbanization,
          city = excluded.city,
          county = excluded.county,
          state2 = excluded.state2,
          szip = excluded.szip,
          lat = excluded.lat,
          lon = excluded.lon,
          inside_x = excluded.inside_x,
          inside_y = excluded.inside_y,
          ll_gisacre = excluded.ll_gisacre,
          ll_gissqft = excluded.ll_gissqft,
          ll_bldg_footprint_sqft = excluded.ll_bldg_footprint_sqft,
          ll_bldg_count = excluded.ll_bldg_count,
          parval = excluded.parval,
          landval = excluded.landval,
          improvval = excluded.improvval,
          taxable = excluded.taxable,
          cabida = excluded.cabida,
          saleprice = excluded.saleprice,
          saledate = excluded.saledate,
          deednum = excluded.deednum,
          book = excluded.book,
          page = excluded.page,
          legaldesc = excluded.legaldesc,
          zoning = excluded.zoning,
          path = excluded.path,
          ll_last_refresh = excluded.ll_last_refresh,
          ll_updated_at = excluded.ll_updated_at,
          source_file_sha256 = excluded.source_file_sha256,
          source_row_sha256 = excluded.source_row_sha256,
          source_row_number = excluded.source_row_number,
          raw_payload = excluded.raw_payload,
          loaded_at = now()
        where current.source_row_sha256 is distinct from excluded.source_row_sha256
        returning ll_uuid
      )
      select count(*)::integer as changed from upserted
    `) as Array<{ changed: number | string }>

    changed += Number(result[0]?.changed ?? 0)
  }

  return {
    accepted: input.rows.length,
    changed,
    replayed: input.rows.length - changed,
  }
}
