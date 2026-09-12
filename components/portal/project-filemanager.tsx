'use client'

import { useEffect, useState } from 'react'
import { Filemanager, WillowDark } from '@svar-ui/react-filemanager'
import type { IEntity } from '@svar-ui/react-filemanager'
import '@svar-ui/react-filemanager/all.css'

// ---------------------------------------------------------------------------
// PROJECTS DOCUMENTS — SVAR Filemanager host for the Documents tab.
//
// Owns the vendor import, the vendor CSS and the dark theme wrapper, so the pane
// keeps no widget plumbing (same shape as ProjectTimeline and FullCalendarCandidate).
//
// Two deliberate choices:
//
// - `readonly` is ON. This is a placeholder surface: an upload, a rename or a
//   delete here must not look like it reached the vault. Nothing on this screen
//   writes to `media` / `property_media`; when document actions become real they
//   go through the Vault service commands, not through the widget.
// - The widget has NO `css` / `class` / `style` passthrough (it stores unknown
//   props for action callbacks and does not spread them), so the container div is
//   ours and the midnight skin is applied to it through the --wx-* variables in
//   globals.css — the same mechanism as .project-timeline.
//
// The tree itself is projected by ui/projects/documents-projection: the project's
// REAL linked documents when it has any, a clearly-flagged sample cabinet when it
// has none.
// ---------------------------------------------------------------------------

export function ProjectFilemanager({ files }: { files: IEntity[] }) {
  const [mounted, setMounted] = useState(false)

  useEffect(() => {
    setMounted(true)
  }, [])

  return (
    <div className="portal-svar-midnight project-filemanager flex h-full min-h-0 w-full flex-col">
      <WillowDark>
        <div className="min-h-[24rem] flex-1 lg:min-h-0">
          {mounted ? (
            <Filemanager data={files} readonly />
          ) : (
            <div className="flex h-full items-center justify-center text-sm font-light text-white/50">
              Loading documents…
            </div>
          )}
        </div>
      </WillowDark>
    </div>
  )
}
