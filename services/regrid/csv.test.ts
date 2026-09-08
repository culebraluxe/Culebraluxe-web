import assert from 'node:assert/strict'
import test from 'node:test'
import { parseCsvRecords, parseRegridCulebraCsv } from './csv'

test('CSV parser handles quoted commas, doubled quotes, CRLF, and embedded JSON', () => {
  const rows = parseCsvRecords(
    'a,b,c\r\n1,"two, too","{ ""address"": ""LOTE 2"" }"\r\n',
  )
  assert.deepEqual(rows, [
    ['a', 'b', 'c'],
    ['1', 'two, too', '{ "address": "LOTE 2" }'],
  ])
})

test('Regrid Culebra parser promotes CRIM fields and preserves raw payload', () => {
  const csv = [
    'll_uuid,parcelnumb,catastro,num_catastro,oldpid,municipio,direccion_fisica,direccion_postal,owner,buyername,address,urbanization,city,county,state2,szip,lat,lon,ll_gisacre,ll_gissqft,ll_bldg_footprint_sqft,ll_bldg_count,parval,landval,improvval,taxable,cabida,saleprice,saledate,deednum,book,page,legaldesc,zoning,path,ll_last_refresh,ll_updated_at',
    '84f4e468-7b4d-4e40-ae00-064b0649970c,473-058-043-03-000,473-058-043-03-000,473-058-043-03,473-000-009-10,Culebra,"BO SAN ISIDRO, LOTE 2, PR",,FELIPPO CORPORATION,FELIPO CORPORATION,LOTE 2,BO SAN ISIDRO,san-isidro,culebra,PR,775,18.32614,-65.267612,5.43146,236599,0,0,1275,1275,0,1275,4.8,0,2014/10/05,10,18,230,,,/us/pr/culebra/san-isidro/299976,2026/05/12,2026/06/07 06:06:34.541+00',
  ].join('\n')

  const parsed = parseRegridCulebraCsv(csv)
  assert.equal(parsed.rows.length, 1)
  assert.equal(parsed.rejected.length, 0)
  assert.equal(parsed.rows[0].catastro, '473-058-043-03-000')
  assert.equal(parsed.rows[0].numCatastro, '473-058-043-03')
  assert.equal(parsed.rows[0].zip, '00775')
  assert.equal(parsed.rows[0].saleDate, '2014-10-05')
  assert.equal(parsed.rows[0].lastRefresh, '2026-05-12')
  assert.equal(parsed.rows[0].rawPayload.direccion_fisica, 'BO SAN ISIDRO, LOTE 2, PR')
})

test('blank Catastro is accepted when the row is still a valid Culebra parcel', () => {
  const csv = [
    'll_uuid,catastro,direccion_fisica,county,municipio,szip,lat,lon',
    '720bbbf2-fbf7-437c-a277-d37337612183,,,culebra,,775,18.294372,-65.291575',
  ].join('\n')
  const parsed = parseRegridCulebraCsv(csv)
  assert.equal(parsed.rows.length, 1)
  assert.equal(parsed.rows[0].catastro, null)
})

test('non-Culebra rows are rejected rather than contaminating the local table', () => {
  const csv = [
    'll_uuid,catastro,direccion_fisica,county,municipio',
    '720bbbf2-fbf7-437c-a277-d37337612183,123,Somewhere,gurabo,Gurabo',
  ].join('\n')
  const parsed = parseRegridCulebraCsv(csv)
  assert.equal(parsed.rows.length, 0)
  assert.equal(parsed.rejected.length, 1)
  assert.match(parsed.rejected[0].reason, /not identified as Culebra/)
})
