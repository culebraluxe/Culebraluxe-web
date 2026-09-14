import { test } from 'node:test'
import assert from 'node:assert/strict'

import { mapProjectToFileTree } from '../ui/projects/documents-projection'
import type { ProjectPlan } from '../ui/projects/model'

const plan = (overrides: Partial<ProjectPlan> = {}): ProjectPlan => ({
  id: 'p1',
  title: 'Sea to Soul Listing',
  kind: 'LISTING',
  status: 'active',
  progress: 42,
  phaseLabel: 'Agreement',
  workNodes: [],
  ...overrides,
})

test('an empty project asset browser is truthful: no sample cabinet is invented', () => {
  const { files, synthetic } = mapProjectToFileTree(plan())
  assert.equal(synthetic, false)
  assert.deepEqual(files, [])
})

test('Vault documents and Property photos share a read model without changing source ownership', () => {
  const { files } = mapProjectToFileTree(plan({
    assets: [
      {
        id: 'vault:d1', sourceId: 'd1', kind: 'document', name: 'Listing Agreement', source: 'vault',
        propertyId: 'prop-1', createdAt: '2026-09-02T00:00:00.000Z', state: 'issued',
      },
      {
        id: 'property-media:m1', sourceId: 'm1', kind: 'photo', name: 'Exterior.jpg', source: 'property-media',
        propertyId: 'prop-1', createdAt: '2026-09-03T00:00:00.000Z', href: '/api/media/m1', caption: 'Front elevation',
      },
    ],
  }))

  assert.equal(files.length, 2)
  assert.equal(files[0]?.source, 'vault')
  assert.equal(files[0]?.kind, 'document')
  assert.equal(files[1]?.source, 'property-media')
  assert.equal(files[1]?.kind, 'photo')
  assert.equal(files[1]?.href, '/api/media/m1')
  assert.equal(files[1]?.caption, 'Front elevation')
})

test('valid source dates become Date values and invalid dates are omitted', () => {
  const { files } = mapProjectToFileTree(plan({
    assets: [
      {
        id: 'vault:good', sourceId: 'good', kind: 'document', name: 'Good.pdf', source: 'vault',
        propertyId: null, createdAt: '2026-09-02T00:00:00.000Z',
      },
      {
        id: 'vault:bad', sourceId: 'bad', kind: 'document', name: 'Bad.pdf', source: 'vault',
        propertyId: null, createdAt: 'not-a-date',
      },
    ],
  }))
  assert.ok(files[0]?.date instanceof Date)
  assert.equal(files[1]?.date, undefined)
})
