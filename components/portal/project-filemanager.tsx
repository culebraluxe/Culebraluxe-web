'use client'

import { useMemo, useState } from 'react'
import { ExternalLink, FileText, Image as ImageIcon } from 'lucide-react'

import type { ProjectAssetBrowserItem } from '@/ui/projects/documents-projection'

type AssetFilter = 'all' | 'document' | 'photo'

const FILTER_LABEL: Record<AssetFilter, string> = {
  all: 'All',
  document: 'Documents',
  photo: 'Photos',
}

function sourceLabel(source: ProjectAssetBrowserItem['source']): string {
  return source === 'vault' ? 'Vault' : 'Property media'
}

function dateLabel(date: Date | undefined): string {
  if (!date) return '—'
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })
}

function kindLabel(item: ProjectAssetBrowserItem): string {
  if (item.kind === 'photo') return 'Photo'
  if (item.mimeType?.toLowerCase().includes('pdf') || /\.pdf$/i.test(item.name)) return 'PDF'
  return 'Document'
}

function AssetIdentity({ item }: { item: ProjectAssetBrowserItem }) {
  const inner = (
    <div className="flex min-w-0 items-center gap-3">
      {item.kind === 'photo' && item.href ? (
        <img
          src={item.href}
          alt={item.altText ?? item.caption ?? item.name}
          className="h-11 w-14 shrink-0 rounded-md border border-white/15 object-cover"
        />
      ) : (
        <span className="flex h-11 w-14 shrink-0 items-center justify-center rounded-md border border-white/10 bg-white/[0.06] text-[var(--portal-gold)]">
          {item.kind === 'photo' ? <ImageIcon className="h-5 w-5" aria-hidden /> : <FileText className="h-5 w-5" aria-hidden />}
        </span>
      )}
      <span className="min-w-0">
        <span className="block truncate text-[14px] font-medium text-white/95">{item.name}</span>
        {item.caption && item.caption !== item.name ? (
          <span className="mt-0.5 block truncate text-[11px] font-light text-white/45">{item.caption}</span>
        ) : null}
      </span>
    </div>
  )

  if (!item.href) return inner
  return (
    <a href={item.href} target="_blank" rel="noreferrer" className="group block min-w-0" title="Open asset">
      {inner}
    </a>
  )
}

/**
 * Read-only project asset browser. It deliberately does not pretend Vault and
 * property media share one physical folder hierarchy; filters are only a view.
 */
export function ProjectFilemanager({ files }: { files: ProjectAssetBrowserItem[] }) {
  const [filter, setFilter] = useState<AssetFilter>('all')
  const counts = useMemo(() => ({
    all: files.length,
    document: files.filter((file) => file.kind === 'document').length,
    photo: files.filter((file) => file.kind === 'photo').length,
  }), [files])
  const visible = useMemo(
    () => filter === 'all' ? files : files.filter((file) => file.kind === filter),
    [files, filter],
  )

  return (
    <section className="flex h-full min-h-0 w-full flex-col overflow-hidden rounded-[var(--portal-tab-radius)] border border-white/20 bg-[color-mix(in_srgb,var(--portal-navy)_94%,transparent)] shadow-sm">
      <div className="flex shrink-0 items-center justify-between gap-3 border-b border-white/10 px-3 py-2">
        <div className="flex items-center gap-1" role="group" aria-label="Asset filter">
          {(Object.keys(FILTER_LABEL) as AssetFilter[]).map((key) => {
            const active = key === filter
            return (
              <button
                key={key}
                type="button"
                onClick={() => setFilter(key)}
                aria-pressed={active}
                className={`rounded-full px-3 py-1.5 text-[11px] font-medium transition ${active ? 'bg-white/15 text-white ring-1 ring-inset ring-white/20' : 'text-white/55 hover:bg-white/[0.07] hover:text-white/90'}`}
              >
                {FILTER_LABEL[key]} <span className="ml-1 text-[10px] opacity-65">{counts[key]}</span>
              </button>
            )
          })}
        </div>
        <span className="hidden text-[10px] font-light uppercase tracking-[0.12em] text-white/35 sm:inline">Read-only · Vault + Property media</span>
      </div>

      <div className="grid shrink-0 grid-cols-[minmax(0,1.7fr)_90px_135px_105px] gap-3 border-b border-white/10 px-3 py-2 text-[9px] font-semibold uppercase tracking-[0.12em] text-white/35">
        <span>Name</span><span>Type</span><span>Source</span><span>Updated</span>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {visible.length === 0 ? (
          <div className="flex h-full min-h-48 items-center justify-center px-6 text-center">
            <p className="text-[13px] font-light text-white/45">
              {files.length === 0 ? 'No project assets are linked yet.' : `No ${FILTER_LABEL[filter].toLowerCase()} are linked to this project.`}
            </p>
          </div>
        ) : (
          <ul className="divide-y divide-white/[0.08]">
            {visible.map((item) => (
              <li key={item.id} className="grid grid-cols-[minmax(0,1.7fr)_90px_135px_105px] items-center gap-3 px-3 py-2.5 transition hover:bg-white/[0.04]">
                <AssetIdentity item={item} />
                <span className="text-[12px] font-light text-white/65">{kindLabel(item)}</span>
                <span className="text-[12px] font-light text-white/65">{sourceLabel(item.source)}</span>
                <span className="flex items-center gap-1 text-[11px] font-light text-white/45">
                  {dateLabel(item.date)}
                  {item.href ? <ExternalLink className="h-3 w-3 opacity-40" aria-hidden /> : null}
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  )
}
