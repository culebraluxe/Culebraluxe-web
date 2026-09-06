'use client'

import { useEffect, useState } from 'react'

import { updateFormAction } from '@/app/portal/forms/actions'
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

export function ListingV4Controls({
  formId,
  personId,
  sellerName,
  templateVersion,
  activeTemplateVersion,
  status,
  issued,
}: {
  formId: string
  personId: string | null
  sellerName: string
  templateVersion: number
  activeTemplateVersion: number
  status: string
  issued: boolean
}) {
  const [selectedPersonId, setSelectedPersonId] = useState(personId)
  const [selectedName, setSelectedName] = useState(sellerName)
  const [pickerOpen, setPickerOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<ClientResult[]>([])
  const [searching, setSearching] = useState(false)
  const [actionBusy, setActionBusy] = useState(false)
  const isActive = templateVersion === activeTemplateVersion
  const locked = issued || status === 'issued'

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

  async function refreshData() {
    if (!selectedPersonId || actionBusy || locked) return
    const controller = getFormEditorController(formId)
    if (!controller) return
    setActionBusy(true)
    try {
      await controller.dispatch({
        operation: 'formEditor.refreshListing',
        payload: { personId: selectedPersonId },
      })
    } finally {
      setActionBusy(false)
    }
  }

  async function updateClientData() {
    if (!selectedPersonId || actionBusy || locked) return
    const controller = getFormEditorController(formId)
    if (!controller) return
    setActionBusy(true)
    try {
      // First flush the exact working JSON draft without touching services.
      const draftSaved = await controller.dispatch({
        operation: 'formEditor.saveDraft',
        payload: { quiet: true },
      })
      if (!draftSaved) return

      const model = controller.snapshot()
      const result = await updateFormAction(
        formId,
        { ...model.values },
        {
          ...model.sections,
          body: model.detailsText,
          bodyEdited: model.bodyEdited ? 'true' : 'false',
        },
      )
      if (!result.ok) {
        await controller.dispatch({
          operation: 'formEditor.feedback',
          payload: { error: result.message },
        })
        return
      }

      const count = result.data.canonicalUpdates?.length ?? 0
      await controller.dispatch({
        operation: 'formEditor.feedback',
        payload: {
          message:
            count > 0
              ? `Client data updated · ${count} canonical fact${count === 1 ? '' : 's'}`
              : 'Client data already current',
          error: null,
        },
      })
    } finally {
      setActionBusy(false)
    }
  }

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

      // Persist the deliberate context switch before a hard refresh so the
      // server recomputes signer candidates and Property context from the newly
      // selected Person. Autosave/draft save is deliberately non-canonical.
      const saved = await controller.dispatch({
        operation: 'formEditor.saveDraft',
        payload: { quiet: true },
      })
      if (!saved) return

      setSelectedPersonId(client.id)
      setSelectedName(client.displayName)
      setPickerOpen(false)
      setQuery('')
      window.location.reload()
    } finally {
      setActionBusy(false)
    }
  }

  return (
    <section className="portal-glass-panel rounded-[var(--portal-panel-radius)] px-3 py-2.5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-serif text-base font-light text-[var(--portal-navy)]">
              Listing Agreement · Template v{templateVersion}
            </span>
            <span className={[
              'rounded-full border px-2 py-0.5 text-[9px] font-medium uppercase tracking-[0.14em]',
              isActive
                ? 'border-[var(--portal-gold)]/50 text-[var(--portal-navy)]'
                : 'border-black/15 text-black/40',
            ].join(' ')}>
              {isActive ? 'Active' : 'History'}
            </span>
            <span className="text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
              {locked ? 'Issued snapshot' : status}
            </span>
          </div>
          <p className="mt-0.5 text-xs font-light text-black/50">
            Seller · {selectedName || 'Not linked'}
            {selectedPersonId ? ' · linked to Client' : ' · select a Client to hydrate'}
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            disabled={locked || actionBusy || !selectedPersonId}
            onClick={() => void refreshData()}
            className="inline-flex min-h-8 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-3 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-35"
          >
            Refresh Data
          </button>
          <button
            type="button"
            disabled={locked || actionBusy || !selectedPersonId}
            onClick={() => void updateClientData()}
            className="inline-flex min-h-8 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-gold)]/60 px-3 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)] transition hover:border-[var(--portal-gold)] disabled:cursor-not-allowed disabled:opacity-35"
          >
            {actionBusy ? 'Working…' : 'Update Client'}
          </button>
          <button
            type="button"
            disabled={locked || actionBusy}
            onClick={() => setPickerOpen((open) => !open)}
            className="inline-flex min-h-8 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[10px] font-medium uppercase tracking-[0.12em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-35"
          >
            {selectedPersonId ? 'Change Client' : 'Select Client'}
          </button>
        </div>
      </div>

      {pickerOpen && !locked ? (
        <div className="mt-2 border-t border-[var(--portal-panel-border)] pt-2">
          <input
            autoFocus
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search clients by name, email, or phone…"
            className="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 py-2 text-sm font-light outline-none placeholder:text-black/30 focus:border-[var(--portal-navy)]"
          />
          <div className="mt-1 max-h-48 overflow-y-auto rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70">
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
                  className="flex w-full items-center justify-between gap-3 border-b border-[var(--portal-panel-border)] px-3 py-2 text-left last:border-b-0 hover:bg-white"
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
    </section>
  )
}
