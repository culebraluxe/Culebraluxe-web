import { Download, ExternalLink, FileText } from 'lucide-react'

import type { PropertyDocument } from '@/lib/property-types'

function formatFileType(mimeType: string, filename: string) {
  if (mimeType === 'application/pdf') return 'PDF'

  const extension = filename.split('.').pop()
  return extension ? extension.toUpperCase() : 'Document'
}

function formatFileSize(bytes?: number | null) {
  if (bytes == null || !Number.isFinite(bytes) || bytes < 0) return null
  if (bytes < 1024) return `${bytes} B`

  const kilobytes = bytes / 1024
  if (kilobytes < 1024) return `${kilobytes.toFixed(1)} KB`

  return `${(kilobytes / 1024).toFixed(1)} MB`
}

export function PropertyDocuments({
  documents,
}: {
  documents: PropertyDocument[]
}) {
  return (
    <div className="mx-auto max-w-5xl">
      <div className="mb-5">
        <p className="text-xs font-medium uppercase tracking-[0.24em] text-brand-gold">
          Property Documents
        </p>
      </div>

      <ul className="space-y-3">
        {documents.map((document) => {
          const route = `/api/media/documents/${document.id}`
          const fileSize = formatFileSize(document.fileSize)

          return (
            <li
              key={document.id}
              className="flex flex-col gap-4 border border-brand-navy/30 bg-card/70 p-4 sm:flex-row sm:items-center sm:justify-between sm:px-5"
            >
              <div className="flex min-w-0 items-center gap-3.5">
                <span className="flex h-10 w-10 flex-none items-center justify-center border border-brand-gold/55 bg-brand-gold/10 text-brand-gold">
                  <FileText className="h-5 w-5" aria-hidden />
                </span>

                <div className="min-w-0">
                  <h3 className="font-serif text-base font-semibold leading-snug text-brand-navy">
                    {document.title}
                  </h3>

                  <p className="mt-1 text-[11px] font-medium uppercase tracking-[0.14em] text-brand-navy/65">
                    {[formatFileType(document.mimeType, document.filename), fileSize]
                      .filter(Boolean)
                      .join(' · ')}
                  </p>
                </div>
              </div>

              <div className="flex flex-none items-center gap-2 sm:justify-end">
                <a
                  href={route}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex min-h-11 items-center justify-center gap-2 border border-brand-navy/35 px-4 text-xs font-semibold uppercase tracking-[0.1em] text-brand-navy transition-colors hover:bg-brand-navy/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold"
                  aria-label={`View ${document.title}`}
                >
                  View
                  <ExternalLink className="h-3.5 w-3.5" aria-hidden />
                </a>

                <a
                  href={`${route}?download=1`}
                  className="inline-flex min-h-11 items-center justify-center gap-2 bg-brand-navy px-4 text-xs font-semibold uppercase tracking-[0.1em] text-brand-ivory transition-colors hover:bg-brand-navy/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold"
                  aria-label={`Download ${document.title}`}
                >
                  Download
                  <Download className="h-3.5 w-3.5" aria-hidden />
                </a>
              </div>
            </li>
          )
        })}
      </ul>
    </div>
  )
}
