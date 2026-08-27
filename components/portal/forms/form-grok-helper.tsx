"use client"

import { useState } from "react"

import { AraMicButton } from "@/components/portal/ara-mic-button"

// ---------------------------------------------------------------------------
// FORMS — Grok / Ara command slot (the COMMAND side of CommandStatusBand).
//
// The typed command path (prompt -> onAsk -> validated Forms mutation seam) is
// UNCHANGED and remains the single source of truth. ARA-VOICE-01: the mic is
// INPUT ONLY — recognized speech fills the SAME editable prompt field used by
// typing, and Go / Enter submits through the same onAsk handler. The mic never
// submits on its own; transcripts stay editable before submission.
// ---------------------------------------------------------------------------

export function FormGrokHelper({
  formTitle,
  busy = false,
  onAsk,
}: {
  formTitle: string
  busy?: boolean
  onAsk: (prompt: string) => Promise<string>
}) {
  const [prompt, setPrompt] = useState("")
  const [note, setNote] = useState<string | null>(null)
  const [working, setWorking] = useState(false)

  // Voice converges into the editable prompt (append, editable before submit).
  const appendTranscript = (text: string) =>
    setPrompt((current) => (current ? `${current} ${text}` : text))

  async function submitPrompt() {
    const text = prompt.trim()
    if (!text) {
      setNote("Tell Grok what happened on the deal, then tap Go.")
      return
    }
    setWorking(true)
    try {
      const reply = await onAsk(text)
      setNote(reply)
    } catch {
      setNote("Grok could not fill the form right now. Try again in a moment.")
    } finally {
      setWorking(false)
    }
  }

  return (
    <div className="w-full">
      <section className="portal-glass-panel rounded-[var(--portal-panel-radius)] px-4 py-3 sm:px-5">
        <div className="flex flex-wrap items-end gap-2">
          <div className="min-w-0 flex-1">
            <p className="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">
              Grok · {formTitle}
            </p>
            <input
              value={prompt}
              onChange={(event) => setPrompt(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault()
                  void submitPrompt()
                }
              }}
              placeholder="Tell Grok what happened — or tap the mic"
              className="mt-1.5 block h-10 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 font-serif text-[15px] font-light text-[var(--portal-navy)] outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
            />
          </div>
          <AraMicButton onTranscript={appendTranscript} ariaLabel="Use microphone to command Grok" />
          <button
            type="button"
            disabled={busy || working}
            onClick={() => {
              void submitPrompt()
            }}
            className="inline-flex h-10 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:opacity-40"
          >
            {working ? "Grok…" : "Go"}
          </button>
        </div>
        <p className="mt-2 text-center text-[11px] font-light text-black/45">
          {note ??
            "Say it like you would to Grok. She can fill the fields; you still Save and Send."}
        </p>
      </section>
    </div>
  )
}
