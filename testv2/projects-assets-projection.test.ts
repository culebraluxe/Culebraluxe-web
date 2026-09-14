import { test } from 'node:test'
import assert from 'node:assert/strict'

import { attachProjectAssets } from '../ui/projects/assets-projection'
import type { ProjectsWorkspaceData } from '../ui/projects/model'
import type { Project } from '../services/project'
import type { WbsItem } from '../services/wbs'

const project = (overrides: Partial<Project> = {}): Project => ({
  id: 'project-1', name: 'Jessica Iverson Listing', owner: null, status: 'doing', description: '', areas: [],
  projectType: 'listing', playbookId: 'listing-onboarding', playbookVersion: 1,
  personId: null, propertyId: 'prop-1', contractId: null, startsAt: null, endsAt: null,
  createdAt: null, updatedAt: null, ...overrides,
})

const workspace = (): ProjectsWorkspaceData => ({
  domains: [{ key: 'properties', label: 'Properties', shortLabel: 'Properties' }],
  poles: [{
    id: 'pole-1', domain: 'properties', label: 'Jessica Iverson Listing', subtitle: '1 project', progress: 0, statusLabel: 'Open',
    projects: [{
      id: 'project-1', title: 'Jessica Iverson Listing', kind: 'LISTING', status: 'active', progress: 0,
      phaseLabel: 'Open', provenance: { documents: 'empty', activity: 'empty', calendar: 'empty' },
      documents: [{ id: 'doc-1', title: 'Listing Agreement', state: 'issued', propertyId: 'prop-1', createdAt: '2026-09-02T00:00:00.000Z' }],
      workNodes: [],
    }],
  }],
})

test('project asset projection combines real Vault documents with only the anchored property photos', () => {
  const data = attachProjectAssets(workspace(), [project()], [], {
    'prop-1': [{
      id: 'media-1', propertyId: 'prop-1', mediaType: 'image', role: 'gallery', sortOrder: 1,
      filename: 'Kitchen.jpg', mimeType: 'image/jpeg', fileSize: 123, altText: null, caption: 'Kitchen',
      createdAt: '2026-09-03T00:00:00.000Z', url: '/api/media/media-1',
    }],
    'prop-2': [{
      id: 'wrong', propertyId: 'prop-2', mediaType: 'image', role: 'gallery', sortOrder: 1,
      filename: 'Wrong.jpg', mimeType: 'image/jpeg', fileSize: 123, altText: null, caption: null,
      createdAt: null, url: '/api/media/wrong',
    }],
  })
  const assets = data.poles[0]?.projects[0]?.assets ?? []
  assert.equal(assets.length, 2)
  assert.deepEqual(new Set(assets.map((asset) => asset.source)), new Set(['vault', 'property-media']))
  assert.ok(!assets.some((asset) => asset.sourceId === 'wrong'))
  assert.equal(data.poles[0]?.projects[0]?.provenance?.documents, 'linked')
})

test('WBS property entity is the fallback anchor when the Project row has no propertyId', () => {
  const item: WbsItem = {
    id: 'w1', title: 'Property', notes: '', category: 'properties', status: 'open', projectId: 'project-1',
    parentId: null, dueAt: null, owner: null, order: 1, entity: { type: 'property', id: 'prop-wbs' },
    createdAt: null, updatedAt: null,
  }
  const data = attachProjectAssets(workspace(), [project({ propertyId: null })], [item], {
    'prop-wbs': [{
      id: 'media-wbs', propertyId: 'prop-wbs', mediaType: 'image', role: 'hero', sortOrder: 0,
      filename: 'Front.jpg', mimeType: 'image/jpeg', fileSize: null, altText: 'Front', caption: null,
      createdAt: null, url: '/api/media/media-wbs',
    }],
  })
  assert.ok(data.poles[0]?.projects[0]?.assets?.some((asset) => asset.sourceId === 'media-wbs'))
})

test('videos are not surfaced in the first project asset browser slice', () => {
  const data = attachProjectAssets(workspace(), [project()], [], {
    'prop-1': [{
      id: 'video-1', propertyId: 'prop-1', mediaType: 'video', role: 'video', sortOrder: 0,
      filename: 'Walkthrough.mp4', mimeType: 'video/mp4', fileSize: 1, altText: null, caption: null,
      createdAt: null, url: '/api/media/video-1',
    }],
  })
  assert.ok(!data.poles[0]?.projects[0]?.assets?.some((asset) => asset.sourceId === 'video-1'))
})
