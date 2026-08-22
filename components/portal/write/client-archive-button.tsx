"use client"

import { useRouter } from "next/navigation"
import { useState, useTransition } from "react"
import { archiveClientAction } from "@/app/portal/actions"

// OPS-02 — Archive (soft delete) a client from the Client Administration
// table. Archive sets person.archived_at; every read projection already
// filters archived_at is null, so the row drops out on the next refresh.
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

  function archive() {
    if (
      !window.confirm(
        `Archive ${displayName}? They will be removed from the client roster, admin table, dossiers, and person search. Their history is preserved.`,
      )
    ) {
      return
    }
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
      <button
        type="button"
        onClick={archive}
        disabled={isPending}
        className="rounded-full border border-[var(--portal-border)] px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.12em] text-[var(--portal-archive)] transition hover:border-[var(--portal-archive)] disabled:cursor-not-allowed disabled:opacity-40"
      >
        {isPending ? "Archiving…" : "Archive"}
      </button>
      {error && (
        <span className="text-[10px] font-light text-[var(--portal-archive)]">{error}</span>
      )}
    </span>
  )
}
