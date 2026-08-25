"use client"

import { useRouter } from "next/navigation"
import { useState, useTransition } from "react"
import { Trash2 } from "lucide-react"
import {
  archivePropertyAction,
  restorePropertyAction,
} from "@/app/portal/actions"
import {
  PortalDialog,
  PortalDialogClose,
} from "@/components/portal/ui/portal-dialog"

// OPS-03 — Archive / Restore (soft delete) for a property. Archive sets
// property.archived_at; every public read projection already filters
// archived_at is null, so the listing drops off the site immediately and is
// fully restorable. The admin index keeps the row (flagged Archived).
// The consequential archive confirmation uses the Portal dialog.

const ghostActionClass =
  "inline-flex min-h-11 items-center justify-center gap-2 rounded-[var(--portal-tab-radius)] px-4 text-[11px] font-medium uppercase tracking-[0.14em] text-[var(--portal-blue-gray)] transition hover:bg-[var(--portal-rail-hover-bg)] hover:text-[var(--portal-navy)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-gold)]/60"

const destructiveActionClass =
  "inline-flex min-h-11 items-center justify-center gap-2 rounded-[var(--portal-tab-radius)] border border-[var(--portal-danger)] px-4 text-[11px] font-medium uppercase tracking-[0.14em] text-[var(--portal-archive)] transition hover:bg-[var(--portal-archive)] hover:text-white focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-danger)]/40 disabled:cursor-not-allowed disabled:opacity-40"

const restorePillClass =
  "rounded-full border border-[var(--portal-border)] px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-40"

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

  function doArchive() {
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
          className={restorePillClass}
        >
          {isPending ? "Restoring…" : "Restore"}
        </button>
      ) : (
        <PortalDialog
          title="Archive property"
          description={`Archive ${name}? It will be removed from the public site immediately. Its record, media links, deals, and history are preserved and can be restored.`}
          trigger="Archive"
          triggerClassName="border-[var(--portal-archive)]/40 text-[var(--portal-archive)] hover:border-[var(--portal-archive)] hover:text-[var(--portal-archive)]"
          actions={
            <>
              <PortalDialogClose className={ghostActionClass}>Cancel</PortalDialogClose>
              <PortalDialogClose
                className={destructiveActionClass}
                onClick={doArchive}
                disabled={isPending}
              >
                <Trash2 className="h-3.5 w-3.5" />
                {isPending ? "Archiving…" : "Archive"}
              </PortalDialogClose>
            </>
          }
        >
          <div className="rounded-[var(--portal-tab-radius)] bg-[var(--portal-archive-pale)] p-4 text-sm font-light leading-6 text-[var(--portal-text)]/75">
            This removes {name} from the public site immediately. The listing, media links, deals,
            and history are preserved and can be restored.
          </div>
        </PortalDialog>
      )}
      {error && (
        <span className="text-[10px] font-light text-[var(--portal-archive)]">{error}</span>
      )}
    </span>
  )
}
