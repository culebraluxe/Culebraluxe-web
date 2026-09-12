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

const ids = (files: Array<{ id: string | number }>): string[] => files.map((f) => String(f.id)).sort()

test('a project with no linked documents gets a sample cabinet, flagged synthetic', () => {
  const { files, synthetic } = mapProjectToFileTree(plan())
  assert.equal(synthetic, true)
  const paths = ids(files)
  // The project folder itself, its sample subfolders, and files that end in .pdf.
  assert.ok(paths.includes('/Sea to Soul Listing'))
  assert.ok(paths.includes('/Sea to Soul Listing/Contracts'))
  assert.ok(paths.some((p) => p.endsWith('.pdf')))
  assert.ok(files.filter((f) => f.type === 'folder').length > 1)
  assert.ok(files.filter((f) => f.type === 'file').length > 0)
})

test('REAL linked documents win: a cabinet with documents contains no sample files', () => {
  const { files, synthetic } = mapProjectToFileTree(
    plan({
      documents: [
        { id: 'd1', title: 'Listing Agreement', state: 'issued', propertyId: 'prop-1', createdAt: '2026-09-02T00:00:00.000Z' },
        { id: 'd2', title: 'Disclosure', state: 'draft', propertyId: 'prop-1', createdAt: '2026-09-03T00:00:00.000Z' },
      ],
    }),
  )
  assert.equal(synthetic, false)
  const paths = ids(files)
  assert.deepEqual(paths, [
    '/Sea to Soul Listing',
    '/Sea to Soul Listing/Draft',
    '/Sea to Soul Listing/Draft/Disclosure.pdf',
    '/Sea to Soul Listing/Issued',
    '/Sea to Soul Listing/Issued/Listing Agreement.pdf',
  ])
  // No sample folders leaked in.
  assert.ok(!paths.includes('/Sea to Soul Listing/Contracts'))
})

test('the vault state drives the folder, and a .pdf title is not double-suffixed', () => {
  const { files } = mapProjectToFileTree(
    plan({
      documents: [
        { id: 'd1', title: 'Signed Agreement.pdf', state: 'issued', propertyId: null, createdAt: '2026-09-02T00:00:00.000Z' },
      ],
    }),
  )
  const file = files.find((f) => f.type === 'file')
  assert.equal(file?.id, '/Sea to Soul Listing/Issued/Signed Agreement.pdf')
})

test('folder names and dates survive as real values, and ids stay path-shaped', () => {
  const { files } = mapProjectToFileTree(
    plan({
      documents: [
        { id: 'd1', title: 'Deed', state: 'issued', propertyId: null, createdAt: '2026-09-02T00:00:00.000Z' },
      ],
    }),
  )
  const file = files.find((f) => f.type === 'file')
  assert.ok(file)
  assert.ok(file.date instanceof Date)
  // The widget derives parent/name/ext from the id, so it must start with '/'.
  assert.ok(String(file.id).startsWith('/'))
})

test('a title containing a path separator cannot escape its folder', () => {
  const { files } = mapProjectToFileTree(
    plan({
      documents: [
        { id: 'd1', title: 'a/b\\c', state: 'issued', propertyId: null, createdAt: '2026-09-02T00:00:00.000Z' },
      ],
    }),
  )
  const file = files.find((f) => f.type === 'file')
  // Exactly three segments: root folder, state folder, file — never more.
  assert.equal(String(file?.id).split('/').length, 4)
  assert.equal(file?.id, '/Sea to Soul Listing/Issued/a-b-c.pdf')
})

test('an unparseable createdAt omits the date instead of inventing one', () => {
  const { files } = mapProjectToFileTree(
    plan({
      documents: [{ id: 'd1', title: 'Deed', state: 'issued', propertyId: null, createdAt: 'not-a-date' }],
    }),
  )
  const file = files.find((f) => f.type === 'file')
  assert.ok(file)
  assert.equal(file.date, undefined)
  assert.ok(!('date' in file))
})
