import test from 'node:test'
import assert from 'node:assert/strict'

import { compareSnapshots, type SchemaSnapshot } from '../lib/schema-parity'

const snap = (over: Partial<SchemaSnapshot> = {}): SchemaSnapshot => ({
  tables: ['project', 'wbs_item'],
  columns: new Map([
    ['project', new Map([['id', 'text'], ['name', 'text']])],
    ['wbs_item', new Map([['id', 'text'], ['project_id', 'text']])],
  ]),
  indexes: new Map([
    ['project.project_pkey', 'CREATE UNIQUE INDEX project_pkey ON public.project USING btree (id)'],
  ]),
  fks: new Map([['wbs_item_project_id_fkey', 'wbs_item -> project']]),
  ...over,
})

test('schema-parity: identical snapshots are clean', () => {
  const report = compareSnapshots(snap(), snap())
  assert.equal(report.clean, true)
  assert.deepEqual(report.tablesOnlyDev, [])
  assert.deepEqual(report.tablesOnlyProd, [])
  assert.deepEqual(report.columnDrift, [])
  assert.deepEqual(report.indexDrift, [])
  assert.deepEqual(report.fkDrift, [])
})

test('schema-parity: a table present on only one side is reported and not clean', () => {
  const devOnly = compareSnapshots(snap({ tables: ['project', 'wbs_item', 'contract'] }), snap())
  assert.deepEqual(devOnly.tablesOnlyDev, ['contract'])
  assert.equal(devOnly.clean, false)

  const prodOnly = compareSnapshots(snap(), snap({ tables: ['project', 'wbs_item', 'agent_work_item'] }))
  assert.deepEqual(prodOnly.tablesOnlyProd, ['agent_work_item'])
  assert.equal(prodOnly.clean, false)
})

test('schema-parity: column differences (presence and type) are reported', () => {
  const dev = snap({
    columns: new Map([
      ['project', new Map([['id', 'text'], ['name', 'text'], ['playbook_version', 'integer']])],
      ['wbs_item', new Map([['id', 'text'], ['project_id', 'text']])],
    ]),
  })
  const report = compareSnapshots(dev, snap())
  assert.equal(report.clean, false)
  assert.ok(report.columnDrift.some((d) => d.includes('playbook_version') && d.includes('DEV-only')))

  const typeDrift = compareSnapshots(
    snap({ columns: new Map([['project', new Map([['id', 'text'], ['name', 'varchar']])], ['wbs_item', new Map([['id', 'text']])]]) }),
    snap(),
  )
  assert.ok(typeDrift.columnDrift.some((d) => d.includes('project.name') && d.includes('DEV=varchar PROD=text')))
})

test('schema-parity: index drift counts as drift (the Forge dispatch lock lives in a partial index)', () => {
  const dev = snap({
    indexes: new Map([
      ['project.project_pkey', 'CREATE UNIQUE INDEX project_pkey ON public.project USING btree (id)'],
      ['agent_work_item.agent_work_item_single_active', 'CREATE UNIQUE INDEX agent_work_item_single_active ON public.agent_work_item USING btree ((true))'],
    ]),
  })
  const report = compareSnapshots(dev, snap())
  assert.equal(report.clean, false)
  assert.ok(report.indexDrift.some((d) => d.includes('agent_work_item_single_active') && d.includes('DEV-only')))
})

test('schema-parity: foreign key differences are reported', () => {
  const report = compareSnapshots(snap({ fks: new Map([['wbs_item_project_id_fkey', 'wbs_item -> wbs_project']]) }), snap())
  assert.equal(report.clean, false)
  assert.ok(report.fkDrift.some((d) => d.includes('wbs_item_project_id_fkey')))
})
