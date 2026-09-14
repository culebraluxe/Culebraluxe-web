import type { ProjectAsset, ProjectPlan } from './model'

// ---------------------------------------------------------------------------
// Project assets -> read-only browser rows (PURE).
//
// The Documents tab is a project-scoped VIEW, not a storage system. Vault
// documents and Property media remain separate authoritative sources; this
// projection only gives them one browseable read model. No fake folders and no
// sample content are invented when the project has no assets.
// ---------------------------------------------------------------------------

export type ProjectAssetBrowserItem = {
  id: string
  assetId: string
  kind: ProjectAsset['kind']
  name: string
  source: ProjectAsset['source']
  href?: string
  state?: string
  caption?: string | null
  altText?: string | null
  mimeType?: string | null
  size?: number | null
  date?: Date
}

export type ProjectDocuments = {
  files: ProjectAssetBrowserItem[]
  /** Kept for the existing pane contract; project assets never fabricate samples. */
  synthetic: false
}

function parseDate(value: string | null | undefined): Date | null {
  if (!value) return null
  const parsed = new Date(value)
  return Number.isNaN(parsed.getTime()) ? null : parsed
}

export function mapProjectToFileTree(project: ProjectPlan): ProjectDocuments {
  const files = (project.assets ?? []).map((asset): ProjectAssetBrowserItem => {
    const date = parseDate(asset.createdAt)
    return {
      id: asset.id,
      assetId: asset.sourceId,
      kind: asset.kind,
      name: asset.name,
      source: asset.source,
      ...(asset.href ? { href: asset.href } : {}),
      ...(asset.state ? { state: asset.state } : {}),
      ...(asset.caption !== undefined ? { caption: asset.caption } : {}),
      ...(asset.altText !== undefined ? { altText: asset.altText } : {}),
      ...(asset.mimeType !== undefined ? { mimeType: asset.mimeType } : {}),
      ...(asset.fileSize !== undefined ? { size: asset.fileSize } : {}),
      ...(date ? { date } : {}),
    }
  })
  return { files, synthetic: false }
}
