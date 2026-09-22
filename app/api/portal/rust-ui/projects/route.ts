import { NextResponse, type NextRequest } from 'next/server'

import {
  rustApiRead,
  rustApiUpdateProject,
  rustApiUpdateWbs,
} from '@/lib/rust-api/client'
import { withApiHandler } from '@/lib/error-capture-seam'

type RustProject = {
  id: string
  name: string
  owner: string | null
  status: string
  description: string
  areas: string[]
  project_type: string | null
  playbook_id: string | null
  playbook_version: number | null
  person_id: string | null
  property_id: string | null
  contract_id: string | null
  starts_at: string | null
  ends_at: string | null
  created_at: string
  updated_at: string
}

type RustWbsItem = {
  id: string
  title: string
  notes: string
  category: string
  status: string
  project_id: string | null
  parent_id: string | null
  due_at: string | null
  owner: string | null
  order: number | null
  entity: { entity_type: string; id: string } | null
  created_at: string | null
  updated_at: string | null
}

type RustMediaAsset = {
  id: string
  propertyId: string
  mediaType: string
  role: string
  sortOrder: number
  filename: string | null
  mimeType: string | null
  fileSize: number | null
  altText: string | null
  caption: string | null
  createdAt: string | null
  url: string
}

type RustVaultDocument = {
  id: string
  propertyId: string | null
  documentTypeLabel: string | null
  title: string | null
  state: string
  templateId: string | null
  templateVersion: number | null
  issuedVersion: number | null
  createdAt: string
  signedArtifactAvailable: boolean
  signedAuditAvailable: boolean
}

type ProjectAction =
  | {
      action: 'projectStatus'
      projectId: string
      status: 'open' | 'doing' | 'done' | 'archived'
    }
  | {
      action: 'wbsSave'
      itemId: string
      title: string
      notes: string
      status: 'open' | 'doing' | 'done' | 'dismissed'
      dueAt: string | null
      owner: string | null
    }

function projectPayload(project: RustProject) {
  return {
    id: project.id,
    name: project.name,
    owner: project.owner,
    status: project.status,
    description: project.description,
    areas: project.areas,
    projectType: project.project_type,
    playbookId: project.playbook_id,
    playbookVersion: project.playbook_version,
    personId: project.person_id,
    propertyId: project.property_id,
    contractId: project.contract_id,
    startsAt: project.starts_at,
    endsAt: project.ends_at,
    createdAt: project.created_at,
    updatedAt: project.updated_at,
  }
}

function wbsPayload(item: RustWbsItem) {
  return {
    id: item.id,
    title: item.title,
    notes: item.notes,
    category: item.category,
    status: item.status,
    projectId: item.project_id,
    parentId: item.parent_id,
    dueAt: item.due_at,
    owner: item.owner,
    order: item.order,
    entity: item.entity
      ? { entityType: item.entity.entity_type, id: item.entity.id }
      : null,
    createdAt: item.created_at,
    updatedAt: item.updated_at,
  }
}

async function resolveIdentityNames(projects: RustProject[], items: RustWbsItem[]) {
  const people = new Set<string>()
  const properties = new Set<string>()
  const contracts = new Set<string>()

  for (const project of projects) {
    if (project.person_id) people.add(project.person_id)
    if (project.property_id) properties.add(project.property_id)
    if (project.contract_id) contracts.add(project.contract_id)
  }
  for (const item of items) {
    const entity = item.entity
    if (!entity) continue
    if (entity.entity_type === 'person') people.add(entity.id)
    if (entity.entity_type === 'property') properties.add(entity.id)
    if (entity.entity_type === 'contract') contracts.add(entity.id)
  }

  const names: Record<string, string> = {}
  await Promise.all([
    ...[...people].map(async (id) => {
      try {
        const result = await rustApiRead<{ display_name?: string }>(
          (`/v1/people/${encodeURIComponent(id)}`) as `/v1/${string}`,
        )
        if (result.value.display_name) names[`person:${id}`] = result.value.display_name
      } catch {
        // The stable id remains the honest fallback in the Yew view.
      }
    }),
    ...[...properties].map(async (id) => {
      try {
        const result = await rustApiRead<{ display_name?: string }>(
          (`/v1/properties/${encodeURIComponent(id)}`) as `/v1/${string}`,
        )
        if (result.value.display_name) names[`property:${id}`] = result.value.display_name
      } catch {
        // Keep the id if this related record is unavailable.
      }
    }),
    ...[...contracts].map(async (id) => {
      try {
        const result = await rustApiRead<{ contract_type?: string }>(
          (`/v1/contracts/${encodeURIComponent(id)}`) as `/v1/${string}`,
        )
        if (result.value.contract_type) {
          names[`contract:${id}`] = result.value.contract_type.replaceAll('_', ' ')
        }
      } catch {
        // Keep the id if this related record is unavailable.
      }
    }),
  ])
  return names
}

function workspacePropertyIds(projects: RustProject[], items: RustWbsItem[]): string[] {
  const ids = new Set<string>()
  for (const project of projects) {
    if (project.property_id) ids.add(project.property_id)
  }
  for (const item of items) {
    if (item.entity?.entity_type === 'property') ids.add(item.entity.id)
  }
  return [...ids]
}

async function workspacePayload() {
  const [projects, items, documents] = await Promise.all([
    rustApiRead<RustProject[]>('/v1/projects'),
    rustApiRead<RustWbsItem[]>('/v1/wbs/project-items'),
    rustApiRead<RustVaultDocument[]>('/v1/vault/documents'),
  ])
  const propertyIds = workspacePropertyIds(projects.value, items.value)
  const [identityNames, mediaByProperty] = await Promise.all([
    resolveIdentityNames(projects.value, items.value),
    Promise.all(
      propertyIds.map(async (id) => {
        try {
          const result = await rustApiRead<RustMediaAsset[]>(
            (`/v1/properties/${encodeURIComponent(id)}/media`) as `/v1/${string}`,
          )
          return result.value
        } catch {
          // Media is supplemental to the project workspace. Vault/WBS still render if one property-media read fails.
          return []
        }
      }),
    ),
  ])
  return {
    projects: {
      projects: projects.value.map(projectPayload),
      items: items.value.map(wbsPayload),
      documents: documents.value.map((document) => ({
        id: document.id,
        propertyId: document.propertyId,
        title: document.title ?? document.documentTypeLabel ?? 'Document',
        state: document.state,
        templateId: document.templateId,
        templateVersion: document.templateVersion,
        issuedVersion: document.issuedVersion,
        createdAt: document.createdAt,
        signedArtifactAvailable: document.signedArtifactAvailable,
        signedAuditAvailable: document.signedAuditAvailable,
      })),
      media: mediaByProperty.flat(),
      identityNames,
    },
  }
}

async function GETHandler(): Promise<Response> {
  return NextResponse.json(await workspacePayload())
}

async function POSTHandler(req: NextRequest): Promise<Response> {
  const input = (await req.json()) as ProjectAction
  if (input.action === 'projectStatus') {
    if (!input.projectId?.trim()) {
      return NextResponse.json({ error: 'projectId is required.' }, { status: 400 })
    }
    await rustApiUpdateProject<RustProject>(input.projectId, { status: input.status })
    return NextResponse.json(await workspacePayload())
  }

  if (input.action === 'wbsSave') {
    if (!input.itemId?.trim()) {
      return NextResponse.json({ error: 'itemId is required.' }, { status: 400 })
    }
    await rustApiUpdateWbs<RustWbsItem>(input.itemId, {
      title: input.title,
      notes: input.notes,
      status: input.status,
      dueAt: input.dueAt,
      owner: input.owner,
    })
    return NextResponse.json(await workspacePayload())
  }

  return NextResponse.json({ error: 'Unsupported Projects action.' }, { status: 400 })
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/projects', route: '/api/portal/rust-ui/projects' },
  GETHandler,
)

export const POST = withApiHandler(
  { label: '/api/portal/rust-ui/projects', route: '/api/portal/rust-ui/projects' },
  POSTHandler,
)
