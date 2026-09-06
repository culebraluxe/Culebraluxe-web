'use client'

import { useEffect, useState } from 'react'

import { getFormEditorController } from '@/ui/form-editor/runtime-bridge'

type ClientResult = {
  id: string
  displayName: string
  primaryEmail?: string | null
  primaryPhone?: string | null
}

type DirectoryResponse = {
  rows?: ClientResult[]
}

/**
 * Compact Listing V4 client-context control.
 *
 * Canonical hydration is automatic on server load/reload. Canonical write-back
 * happens through the existing Save/Send boundary. The only extra operator
 * action needed in the form is changing which Client owns the Listing draft.
 */
export function ListingV4Controls({
  formId,
  personId,
  locked,
}: {
  formId: string
  personId: string | null
  locked: boolean
}) {
  const [pickerOpen, setPickerOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<ClientResult[]>([])
  const [searching, setSearching] = useState(false)
  const [actionBusy, setActionBusy] = useState(false)

  useEffect(() => {
    if (!pickerOpen) return
    const needle = query.trim()
    const abort = new AbortController()
    const timer = window.setTimeout(async () => {
      setSearching(true)
      try {
        const params = new URLSearchParams({
          view: 'directory',
          search: needle,
          page: '1',
          pageSize: '8',
          sort: 'name',
        })
        const response = await fetch(`/api/portal/clients?${params.toString()}`, {
          signal: abort.signal,
        })
        if (!response.ok) throw new Error(`Client search HTTP ${response.status}`)
        const body = (await response.json()) as DirectoryResponse
        setResults(body.rows ?? [])
      } catch (error) {
        if (!(error instanceof DOMException && error.name === 'AbortError')) {
          setResults([])
        }
      } finally {
        if (!abort.signal.aborted) setSearching(false)
      }
    }, 220)
    return () => {
      window.clearTimeout(timer)
      abort.abort()
    }
  }, [pickerOpen, query])

  async function selectClient(client: ClientResult) {
    if (actionBusy || locked) return
    const controller = getFormEditorController(formId)
    if (!controller) return
    setActionBusy(true)
    try {
      const linked = await controller.dispatch({
        operation: 'formEditor.selectListingClient',
        payload: { personId: client.id },
      })
      if (!linked) return

      // Persist the deliberate context switch as working-draft state only.
      // Reload then performs the normal automatic canonical hydration and
      // rebuilds signer candidates from the newly selected Person.
      const saved = await controller.dispatch({
        operation: 'formEditor.saveDraft',
        payload: { quiet: true },
      })
      if (!saved) return

      setPickerOpen(false)
      setQuery('')
      window.location.reload()
    } finally {
      setActionBusy(false)
    }
  }

  return (
    <div className="relative">
      <button
        type="button"
        disabled={locked || actionBusy}
        onClick={() => setPickerOpen((open) => !open)}
        className="inline-flex min-h-7 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-2.5 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-35"
      >
        {actionBusy ? 'Working…' : personId ? 'Change Client' : 'Select Client'}
      </button>

      {pickerOpen && !locked ? (
        <div className="absolute left-0 top-full z-30 mt-1 w-[min(28rem,75vw)] rounded-[var(--portal-panel-radius)] border border-[var(--portal-panel-border)] bg-white/95 p-2 shadow-lg backdrop-blur">
          <input
            autoFocus
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search clients by name, email, or phone…"
            className="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-3 py-2 text-sm font-light outline-none placeholder:text-black/30 focus:border-[var(--portal-navy)]"
          />
          <div className="mt-1 max-h-48 overflow-y-auto rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white">
            {searching ? (
              <p className="px-3 py-2 text-xs font-light text-black/40">Searching…</p>
            ) : results.length === 0 ? (
              <p className="px-3 py-2 text-xs font-light text-black/40">No matching clients.</p>
            ) : (
              results.map((client) => (
                <button
                  key={client.id}
                  type="button"
                  onClick={() => void selectClient(client)}
                  className="flex w-full items-center justify-between gap-3 border-b border-[var(--portal-panel-border)] px-3 py-2 text-left last:border-b-0 hover:bg-black/[0.025]"
                >
                  <span className="text-sm font-medium text-[var(--portal-navy)]">
                    {client.displayName}
                  </span>
                  <span className="truncate text-[11px] font-light text-black/40">
                    {client.primaryEmail || client.primaryPhone || 'Client'}
                  </span>
                </button>
              ))
            )}
          </div>
        </div>
      ) : null}
    </div>
  )
}
