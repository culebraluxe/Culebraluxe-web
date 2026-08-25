"use client"

import { useEffect, useRef, useState, useTransition } from "react"

import { updatePersonNotesAction } from "@/app/portal/actions"
import { Panel } from "@/components/portal/panel"

// ---------------------------------------------------------------------------
// CLIENTS — Notes: a simple, durable, selected-client free-form note.
//
// One editable body of client notes ("What do I want to remember about this
// client?"), persisted through the existing canonical person-notes seam
// (updatePersonNotesAction -> person.notes) — NOT a second CRM/note subsystem
// and never a fake communication event. Durable, person-scoped, editable, and
// reloaded with the selected client. No channel / title / date / manual-log
// controls.
// ---------------------------------------------------------------------------

const saveBtn =
  "inline-flex min-h-9 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40"

export function ClientNotes({
  personId,
  initialNotes,
}: {
  personId: string
  initialNotes?: string
}) {
  const [text, setText] = useState(initialNotes ?? "")
  const [status, setStatus] = useState<{ ok: boolean; text: string } | null>(
    null
  )
  const [isPending, startTransition] = useTransition()
  const savedRef = useRef<string | null>(initialNotes ?? null)

  // Reset when a different client is selected (the component is reused).
  useEffect(() => {
    setText(initialNotes ?? "")
    setStatus(null)
    savedRef.current = initialNotes ?? null
  }, [personId, initialNotes])

  const dirty = text !== (savedRef.current ?? "")

  function save() {
    setStatus(null)
    startTransition(async () => {
      const result = await updatePersonNotesAction(personId, text)
      if (result.ok) {
        savedRef.current = text
        setStatus({ ok: true, text: "Saved" })
      } else {
        setStatus({
          ok: false,
          text: result.message ?? "Could not save.",
        })
      }
    })
  }

  return (
    <Panel compact heading="Notes" className="flex h-full min-h-0 flex-col">
      <textarea
        value={text}
        onChange={(event) => setText(event.target.value)}
        placeholder="What do I want to remember about this client?"
        aria-label="Client notes"
        className="min-h-[9rem] w-full flex-1 resize-none rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 p-3 font-serif text-[15px] font-light leading-6 text-[var(--portal-navy)] outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
      />
      <div className="mt-3 flex shrink-0 items-center justify-between gap-2">
        <span
          aria-live="polite"
          className={`text-[10px] font-light ${
            status && !status.ok
              ? "text-[var(--portal-archive)]"
              : "text-black/45"
          }`}
        >
          {status?.text ?? ""}
        </span>
        <button
          type="button"
          disabled={isPending || !dirty}
          onClick={save}
          className={saveBtn}
        >
          Save
        </button>
      </div>
    </Panel>
  )
}
