'use client'

import { useState } from 'react'

import { AraMicButton } from '@/components/portal/ara-mic-button'

// ---------------------------------------------------------------------------
// CATCH-UP — Ara command slot (the COMMAND side of CommandStatusBand).
//
// Ara is the more useful side of the band. Client/AI orchestration is a staged
// capability; this surface establishes the clean, honest seam without faking
// capabilities. The screen reuses CommandStatusBand (command-6040) — no local
// ratio CSS, no duplicated Forms/Clients implementation.
//
// ARA-VOICE-01: the microphone is INPUT ONLY — recognized speech fills the SAME
// editable prompt field used by typing, and Go / Enter submits through the same
// onRun handler. The mic never submits on its own.
// ---------------------------------------------------------------------------

export function CatchUpCommand({
  onRun,
}: {
  onRun: (prompt: string) => void
}) {
  const [prompt, setPrompt] = useState('')
  const [note, setNote] = useState<string | null>(null)

  const submit = () => {
    const text = prompt.trim()
    if (!text) {
      setNote('Ask Ara who needs you today, then tap Go.')
      return
    }
    onRun(text)
    setPrompt('')
    setNote('Ara noted it — deeper understanding is a staged capability.')
  }

  // Voice converges into the editable prompt (append, editable before submit).
  const appendTranscript = (text: string) =>
    setPrompt((current) => (current ? `${current} ${text}` : text))

  return (
    <section className="portal-glass-panel portal-glass-panel-lifted rounded-[var(--portal-panel-radius)] px-4 py-3 sm:px-5">
      <div className="flex flex-wrap items-end gap-2">
        <div className="min-w-0 flex-1">
          <p className="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">
            Ara · Command
          </p>
          <input
            value={prompt}
            onChange={(event) => setPrompt(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                event.preventDefault()
                submit()
              }
            }}
            placeholder='Who needs me today? e.g. "Who should I follow up with before Friday?"'
            className="mt-1.5 block h-10 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 font-serif text-[15px] font-light text-[var(--portal-navy)] outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
          />
        </div>
        <AraMicButton onTranscript={appendTranscript} ariaLabel="Use microphone to command Ara" />
        <button
          type="button"
          onClick={submit}
          className="inline-flex h-10 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)]"
        >
          Go
        </button>
      </div>
      <p className="mt-2 text-center text-[11px] font-light text-black/45">
        {note ??
          'The queue below already answers who needs you — Ara makes acting on it conversational over time.'}
      </p>
    </section>
  )
}
