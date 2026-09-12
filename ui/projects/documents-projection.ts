import type { IEntity } from '@svar-ui/react-filemanager'

import type { ProjectPlan } from './model'

// ---------------------------------------------------------------------------
// Project documents -> SVAR Filemanager tree (PURE).
//
// The Filemanager's data model is a FLAT list of entities whose `id` is a path
// ('/Cabinet/Contracts/Agreement.pdf'); it derives parent/name/ext from that id,
// and creates the root '/' itself. So this module's whole job is to turn the
// project's real documents into path ids, and to add the folders that give the
// cabinet its shape.
//
// Real documents win. When a project HAS linked documents, the tree shows exactly
// those and nothing else. When it has none, a sample cabinet is produced so the
// tab is judgeable at all — and the tree is flagged `synthetic` so the pane
// DISCLOSES it rather than passing sample files off as the client's real papers.
//
// `IEntity` is a TYPE-ONLY import, so this stays runtime-pure for the glass-box tier.
// ---------------------------------------------------------------------------

/** Sample cabinet, used only for a project with no linked documents yet. */
const SAMPLE_FILES: Array<{ folder: string; name: string; size: number; date: string }> = [
  { folder: 'Contracts', name: 'Listing Agreement.pdf', size: 284_512, date: '2026-09-02T00:00:00.000Z' },
  { folder: 'Contracts', name: 'Seller Signature.pdf', size: 96_740, date: '2026-09-04T00:00:00.000Z' },
  { folder: 'Disclosures', name: 'Property Disclosure.pdf', size: 431_208, date: '2026-09-03T00:00:00.000Z' },
  { folder: 'Marketing', name: 'Coming Soon Flyer.pdf', size: 1_208_344, date: '2026-09-06T00:00:00.000Z' },
  { folder: 'Photos', name: 'Exterior.jpg', size: 2_884_200, date: '2026-09-05T00:00:00.000Z' },
]

export type ProjectDocuments = {
  files: IEntity[]
  /** True when the tree contains sample files rather than only the project's own. */
  synthetic: boolean
}

/** Path segments must not smuggle separators into an id. */
function segment(value: string): string {
  return value.replace(/[/\\]+/g, '-').trim() || 'untitled'
}

function folder(id: string): IEntity {
  return { id, type: 'folder' }
}

function parseDate(value: string | undefined): Date | null {
  if (!value) return null
  const parsed = new Date(value)
  return Number.isNaN(parsed.getTime()) ? null : parsed
}

/** 'issued' -> 'Issued'; anything unmappable falls back to the raw value. */
function stateLabel(state: string): string {
  const trimmed = state.trim()
  if (!trimmed) return 'Unsorted'
  return trimmed.charAt(0).toUpperCase() + trimmed.slice(1)
}

export function mapProjectToFileTree(project: ProjectPlan): ProjectDocuments {
  const rootName = segment(project.title)
  const rootId = `/${rootName}`
  const documents = project.documents ?? []

  if (documents.length === 0) {
    const folders = new Set<string>()
    const files: IEntity[] = [folder(rootId)]
    for (const sample of SAMPLE_FILES) {
      const folderId = `${rootId}/${sample.folder}`
      if (!folders.has(folderId)) {
        folders.add(folderId)
        files.push(folder(folderId))
      }
      const date = parseDate(sample.date)
      files.push({
        id: `${folderId}/${sample.name}`,
        type: 'file',
        size: sample.size,
        ...(date ? { date } : {}),
      })
    }
    return { files, synthetic: true }
  }

  const folders = new Set<string>()
  const files: IEntity[] = [folder(rootId)]
  for (const document of documents) {
    const folderId = `${rootId}/${stateLabel(document.state)}`
    if (!folders.has(folderId)) {
      folders.add(folderId)
      files.push(folder(folderId))
    }
    const date = parseDate(document.createdAt)
    // The vault stores issued documents as PDFs; the extension drives the icon
    // and the preview in the widget, so it has to be on the id.
    const name = /\.pdf$/i.test(document.title) ? document.title : `${document.title}.pdf`
    files.push({
      id: `${folderId}/${segment(name)}`,
      type: 'file',
      ...(date ? { date } : {}),
    })
  }
  return { files, synthetic: false }
}
