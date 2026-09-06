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

function normalizedName(value: string) {
  return value
    .trim()
    .toLocaleLowerCase()
    .replace(/[’']/g, '')
    .replace(/\s+/g, ' ')
}

export function ListingV4Controls({
  formId,
  personId: _personId,
  locked,
}: {
  formId: string
  personId: string | null
  locked: boolean
}) {
  const [actionBusy, setActionBusy] = useState(false)
  const [matches, setMatches] = useState<ClientResult[]>([])

  async function feedback(next: { message?: string | null; error?: string | null }) {
    const controller = getFormEditorController(formId)
    if (!controller) return
    await controller.dispatch({
      operation: 'formEditor.feedback',
      payload: next,
    })
  }

  async function linkClient(client: ClientResult) {
    const controller = getFormEditorController(formId)
    if (!controller || locked) return

    const linked = await controller.dispatch({
      operation: 'formEditor.selectListingClient',
      payload: { personId: client.id },
    })
    if (!linked) return

    // Selection changes only the working Listing context + canonical-owned form
    // fields. Persist that JSON draft, then reload so the server recomputes
    // signer candidates and the explicit Person/Property context.
    const saved = await controller.dispatch({
      operation: 'formEditor.saveDraft',
      payload: { quiet: true },
    })
    if (!saved) return

    setMatches([])
    window.location.reload()
  }

  async function fillFromSeller(rawName?: string) {
    if (locked || actionBusy) return
    const controller = getFormEditorController(formId)
    if (!controller) return

    const sellerName = (rawName ?? controller.snapshot().values.sellerName ?? '').trim()
    if (!sellerName) {
      await feedback({ error: 'Enter the seller name first.' })
      return
    }

    setActionBusy(true)
    setMatches([])
    try {
      const params = new URLSearchParams({
        view: 'directory',
        search: sellerName,
        page: '1',
        pageSize: '8',
        sort: 'name',
      })
      const response = await fetch(`/api/portal/clients?${params.toString()}`, {
        cache: 'no-store',
      })
      if (!response.ok) {
        throw new Error(`Client search HTTP ${response.status}`)
      }
      const body = (await response.json()) as DirectoryResponse
      const rows = body.rows ?? []
      const needle = normalizedName(sellerName)
      const exact = rows.filter(
        (client) => normalizedName(client.displayName) === needle,
      )

      // The normal path is intentionally one-step: Enter after the seller name
      // resolves an exact match, or the only plausible directory result, and
      // immediately hydrates the Listing. A chooser appears only when the name
      // is genuinely ambiguous.
      const candidate = exact.length === 1
        ? exact[0]
        : rows.length === 1
          ? rows[0]
          : null

      if (candidate) {
        await linkClient(candidate)
        return
      }

      if (rows.length === 0) {
        await feedback({
          error: `No Client found for “${sellerName}”. Check the name and try again.`,
        })
        return
      }

      setMatches(exact.length > 1 ? exact : rows)
      await feedback({
        message: `More than one Client matches “${sellerName}” · choose one below`,
        error: null,
      })
    } catch (error) {
      await feedback({
        error: error instanceof Error ? error.message : 'Could not look up the Client.',
      })
    } finally {
      setActionBusy(false)
    }
  }

  useEffect(() => {
    if (locked) return

    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== 'Enter' || event.metaKey || event.ctrlKey || event.altKey) return
      const target = event.target
      if (!(target instanceof HTMLInputElement)) return

      // Reuse the existing proven Seller input; do not introduce a second
      // client-picker field. The exact label check avoids treating Seller
      // Residence Address (or any future Seller-* field) as the lookup trigger.
      const label = target.closest('label')
      const heading = label?.querySelector('span')?.textContent?.trim() ?? ''
      const fieldLabel = heading.replace(/\s*\*\s*$/, '').trim().toLocaleLowerCase()
      if (fieldLabel !== 'seller') return

      event.preventDefault()
      event.stopPropagation()
      void fillFromSeller(target.value)
    }

    document.addEventListener('keydown', onKeyDown, true)
    return () => document.removeEventListener('keydown', onKeyDown, true)
  }, [actionBusy, formId, locked])

  return (
    <div className="relative order-first flex items-center">
      <button
        type="button"
        disabled={locked || actionBusy}
        onClick={() => void fillFromSeller()}
        className="inline-flex min-h-7 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-2.5 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-35"
        title="Fill this Listing from the seller Client"
      >
        {actionBusy ? 'Filling…' : 'Fill Client'}
      </button>

      {matches.length > 0 ? (
        <div className="absolute left-0 top-full z-30 mt-1 w-72 overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white shadow-lg">
          {matches.map((client) => (
            <button
              key={client.id}
              type="button"
              disabled={actionBusy}
              onClick={() => {
                setActionBusy(true)
                void linkClient(client).finally(() => setActionBusy(false))
              }}
              className="flex w-full items-center justify-between gap-3 border-b border-[var(--portal-panel-border)] px-3 py-2 text-left last:border-b-0 hover:bg-black/[0.03] disabled:opacity-40"
            >
              <span className="truncate text-[12px] font-medium text-[var(--portal-navy)]">
                {client.displayName}
              </span>
              <span className="max-w-32 truncate text-[10px] font-light text-black/40">
                {client.primaryEmail || client.primaryPhone || 'Client'}
              </span>
            </button>
          ))}
        </div>
      ) : null}
    </div>
  )
}
