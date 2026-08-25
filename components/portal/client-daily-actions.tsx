'use client'

import { useState, useTransition } from 'react'
import { randomUUID } from 'crypto'

import { ContactActions } from '@/components/portal/contact-actions'
import {
  recordOutcomeAction,
  createQuickNextActionAction,
} from '@/app/portal/actions'
import { NEXT_ACTION_PRESET_CODES } from '@/lib/relationship-intel/next-action-presets'

// ---------------------------------------------------------------------------
// CORE-DAILY-09 — daily working surface for a Client: contact, record outcome,
// establish next action. Uses canonical commands; contact launch alone never
// records success. No forced navigation into Dossier for routine work.
// ---------------------------------------------------------------------------

const outcomes: Array<{ code: string; label: string }> = [
  { code: 'connected', label: 'Connected' },
  { code: 'no_answer', label: 'No answer' },
  { code: 'left_message', label: 'Left message' },
  { code: 'sent_information', label: 'Sent info' },
]

const btn =
  'inline-flex min-h-11 items-center justify-center rounded-[var(--portal-tab-radius)] px-3 text-[11px] font-light uppercase tracking-[0.12em] transition disabled:opacity-40'
const btnSolid =
  'bg-[var(--portal-navy)] text-white hover:bg-[var(--portal-navy-soft)]'
const btnGhost =
  'border border-[var(--portal-border)] text-[var(--portal-navy-soft)] hover:border-[var(--portal-navy)]'

export function ClientDailyActions({
  clientId,
  email,
  phone,
  dealId,
}: {
  clientId: string
  email?: string | null
  phone?: string | null
  dealId?: string | null
}) {
  const [isPending, startTransition] = useTransition()
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null)

  const run = (fn: () => Promise<unknown>) => {
    setMessage(null)
    startTransition(async () => {
      const r = (await fn()) as { ok: boolean; message?: string }
      if (!r.ok) setMessage({ ok: false, text: r.message ?? 'Action failed.' })
    })
  }

  const recordOutcome = (outcome: string) =>
    run(() =>
      recordOutcomeAction({
        commandId: randomUUID(),
        personId: clientId,
        channel: 'call',
        outcome,
        dealId,
        source: 'client_manager',
      }),
    )

  const nextAction = (preset: string) =>
    run(() =>
      createQuickNextActionAction({
        commandId: randomUUID(),
        personId: clientId,
        dealId,
        preset,
        source: 'client_manager',
      }),
    )

  return (
    <div className="space-y-3">
      <ContactActions
        evidence={{ emails: email ? [email] : [], phones: phone ? [phone] : [] }}
        personName=""
      />
      <div className="flex flex-wrap items-center gap-1.5">
        {outcomes.map((o) => (
          <button key={o.code} type="button" disabled={isPending} onClick={() => recordOutcome(o.code)} className={`${btn} ${btnSolid}`}>
            {o.label}
          </button>
        ))}
      </div>
      <div className="flex flex-wrap items-center gap-1.5">
        {NEXT_ACTION_PRESET_CODES.slice(0, 5).map((p) => (
          <button key={p} type="button" disabled={isPending} onClick={() => nextAction(p)} className={`${btn} ${btnGhost}`}>
            {p.replace(/_/g, ' ')}
          </button>
        ))}
      </div>
      {message && !message.ok ? (
        <span className="text-[10px] font-light text-[var(--portal-archive)]">{message.text}</span>
      ) : null}
    </div>
  )
}
