"use client"

import { useRouter } from "next/navigation"
import { useState, useTransition } from "react"
import { Trash2 } from "lucide-react"
import { archiveClientAction } from "@/app/portal/actions"
import {
  PortalDialog,
  PortalDialogClose,
} from "@/components/portal/ui/portal-dialog"

// OPS-02 — Archive (soft delete) a client from the Client Administration
// table. Archive sets person.archived_at; every read projection already
// filters archived_at is null, so the row drops out on the next refresh.
// The consequential confirmation uses the Portal dialog instead of the
// browser-native window.confirm.

const ghostActionClass =
  "inline-flex min-h-11 items-center justify-center gap-2 rounded-[var(--portal-tab-radius)] px-4 text-[11px] font-medium uppercase tracking-[0.14em] text-[var(--portal-blue-gray)] transition hover:bg-[var(--portal-rail-hover-bg)] hover:text-[var(--portal-navy)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-gold)]/60"

const destructiveActionClass =
  "inline-flex min-h-11 items-center justify-center gap-2 rounded-[var(--portal-tab-radius)] border border-[var(--portal-danger)] px-4 text-[11px] font-medium uppercase tracking-[0.14em] text-[var(--portal-archive)] transition hover:bg-[var(--portal-archive)] hover:text-white focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-danger)]/40 disabled:cursor-not-allowed disabled:opacity-40"

export function ClientArchiveButton({
  personId,
  displayName,
}: {
  personId: string
  displayName: string
}) {
  const router = useRouter()
  const [isPending, startTransition] = useTransition()
  const [error, setError] = useState<string | null>(null)

  function doArchive() {
    setError(null)
    startTransition(async () => {
      const result = await archiveClientAction(personId)
      if (!result.ok) {
        setError(result.message)
        return
      }
      router.refresh()
    })
  }

  return (
    <span className="inline-flex flex-col items-start gap-1">
      <PortalDialog
        title="Archive client"
        description={`Archive ${displayName}? They will be removed from the client roster, admin table, dossiers, and person search. Their history is preserved.`}
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
          This removes {displayName} from the active roster. Their record and history are
          preserved and can be restored.
        </div>
      </PortalDialog>
      {error && (
        <span className="text-[10px] font-light text-[var(--portal-archive)]">{error}</span>
      )}
    </span>
  )
}
