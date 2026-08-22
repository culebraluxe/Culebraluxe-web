"use client"

import { useRouter } from "next/navigation"
import { useState, useTransition } from "react"
import {
  archivePropertyAction,
  restorePropertyAction,
} from "@/app/portal/actions"

// OPS-03 — Archive / Restore (soft delete) for a property. Archive sets
// property.archived_at; every public read projection already filters
// archived_at is null, so the listing drops off the site immediately and is
// fully restorable. The admin index keeps the row (flagged Archived).
export function PropertyArchiveButton({
  propertyId,
  name,
  archived,
}: {
  propertyId: string
  name: string
  archived: boolean
}) {
  const router = useRouter()
  const [isPending, startTransition] = useTransition()
  const [error, setError] = useState<string | null>(null)

  function archive() {
    if (
      !window.confirm(
        `Archive ${name}? It will be removed from the public site immediately. Its record, media links, deals, and history are preserved and can be restored.`,
      )
    ) {
      return
    }
    setError(null)
    startTransition(async () => {
      const result = await archivePropertyAction(propertyId)
      if (!result.ok) {
        setError(result.message)
        return
      }
      router.refresh()
    })
  }

  function restore() {
    setError(null)
    startTransition(async () => {
      const result = await restorePropertyAction(propertyId)
      if (!result.ok) {
        setError(result.message)
        return
      }
      router.refresh()
    })
  }

  return (
    <span className="inline-flex flex-col items-start gap-1">
      {archived ? (
        <button
          type="button"
          onClick={restore}
          disabled={isPending}
          className="rounded-full border border-[var(--portal-border)] px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-40"
        >
          {isPending ? "Restoring…" : "Restore"}
        </button>
      ) : (
        <button
          type="button"
          onClick={archive}
          disabled={isPending}
          className="rounded-full border border-[var(--portal-border)] px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.12em] text-[var(--portal-archive)] transition hover:border-[var(--portal-archive)] disabled:cursor-not-allowed disabled:opacity-40"
        >
          {isPending ? "Archiving…" : "Archive"}
        </button>
      )}
      {error && (
        <span className="text-[10px] font-light text-[var(--portal-archive)]">{error}</span>
      )}
    </span>
  )
}
