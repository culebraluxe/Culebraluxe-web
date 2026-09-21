import type { MediaAssetDto } from '@/legacy/services/media'
import type { Project } from '@/legacy/services/project'
import type { WbsItem } from '@/legacy/services/wbs'
import type { ProjectAsset, ProjectPlan, ProjectsWorkspaceData } from './model'

function projectPropertyIds(project: Project | undefined, items: readonly WbsItem[]): string[] {
  if (project?.propertyId) return [project.propertyId]
  const ids = new Set<string>()
  for (const item of items) {
    if (item.projectId === project?.id && item.entity?.type === 'property') ids.add(item.entity.id)
  }
  return Array.from(ids)
}

function assetDate(asset: ProjectAsset): string {
  return asset.createdAt ?? ''
}

function projectAssets(
  plan: ProjectPlan,
  propertyIds: readonly string[],
  mediaByPropertyId: Readonly<Record<string, readonly MediaAssetDto[]>>,
): ProjectAsset[] {
  const assets: ProjectAsset[] = []
  const seen = new Set<string>()

  for (const document of plan.documents ?? []) {
    const id = `vault:${document.id}`
    if (seen.has(id)) continue
    seen.add(id)
    assets.push({
      id,
      sourceId: document.id,
      kind: 'document',
      name: document.title,
      source: 'vault',
      propertyId: document.propertyId,
      createdAt: document.createdAt,
      state: document.state,
    })
  }

  for (const propertyId of propertyIds) {
    for (const media of mediaByPropertyId[propertyId] ?? []) {
      if (media.mediaType !== 'image') continue
      const id = `property-media:${media.id}`
      if (seen.has(id)) continue
      seen.add(id)
      const name =
        media.filename?.trim() ||
        media.caption?.trim() ||
        media.altText?.trim() ||
        'Property photo'
      assets.push({
        id,
        sourceId: media.id,
        kind: 'photo',
        name,
        source: 'property-media',
        propertyId: media.propertyId,
        createdAt: media.createdAt,
        href: media.url,
        caption: media.caption,
        altText: media.altText,
        mimeType: media.mimeType,
        fileSize: media.fileSize,
      })
    }
  }

  return assets.sort(
    (a, b) => assetDate(b).localeCompare(assetDate(a)) || a.name.localeCompare(b.name),
  )
}

/**
 * Add the Documents-tab project asset projection without changing storage truth:
 * Vault documents stay Vault documents; property photos stay media/property_media.
 */
export function attachProjectAssets(
  workspace: ProjectsWorkspaceData,
  projects: readonly Project[],
  items: readonly WbsItem[],
  mediaByPropertyId: Readonly<Record<string, readonly MediaAssetDto[]>>,
): ProjectsWorkspaceData {
  const projectsById = new Map(projects.map((project) => [project.id, project]))

  return {
    ...workspace,
    poles: workspace.poles.map((pole) => ({
      ...pole,
      projects: pole.projects.map((plan) => {
        const project = projectsById.get(plan.id)
        const propertyIds = projectPropertyIds(project, items)
        const assets = projectAssets(plan, propertyIds, mediaByPropertyId)
        const documentsStatus = propertyIds.length === 0
          ? 'unlinked' as const
          : assets.length > 0
            ? 'linked' as const
            : 'empty' as const
        return {
          ...plan,
          assets,
          provenance: plan.provenance
            ? { ...plan.provenance, documents: documentsStatus }
            : plan.provenance,
        }
      }),
    })),
  }
}
