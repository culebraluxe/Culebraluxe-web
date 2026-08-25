'use client'

import { useState, useTransition } from 'react'
import { randomUUID } from 'crypto'

import {
  applyFollowUpCommandAction,
  createQuickNextActionAction,
  recordOutcomeAction,
} from '@/app/portal/actions'

// ---------------------------------------------------------------------------
// CORE-DAILY-06 — Done / Done + next touch / Snooze for a relationship
// follow-up. Deterministic and replay-safe (each command carries an idempotent
// commandId). Snooze never touches workflow/legal deadlines.
// ---------------------------------------------------------------------------

const snoozePresets: Array<{ label: string; offsetMs: number }> = [
  { label: 'Later today', offsetMs: 4 * 60 * 60 * 1000 },
  { label: 'Tomorrow', offsetMs: 24 * 60 * 60 * 1000 },
  { label: 'Next week', offsetMs: 7 * 24 * 60 * 60 * 1000 },
]

const outcomes: Array<{ code: string; label: string }> = [
  { code: 'connected', label: 'Connected' },
  { code: 'no_answer', label: 'No answer' },
  { code: 'left_message', label: 'Left message' },
  { code: 'sent_information', label: 'Sent info' },
]

const btn =
  'inline-flex min-h-11 items-center justify-center rounded-[var(--portal-tab-radius)] px-3 text-[11px] font-light uppercase tracking-[0.12em] transition disabled:opacity-40'
const btnDone =
  'bg-[var(--portal-navy)] text-white hover:bg-[var(--portal-navy-soft)]'
const btnGhost =
  'border border-[var(--portal-border)] text-[var(--portal-navy-soft)] hover:border-[var(--portal-navy)]'

export function FollowUpActions({
  followUpId,
  personId,
  propertyId,
  dealId,
  personName,
}: {
  followUpId: string
  personId: string
  propertyId?: string | null
  dealId?: string | null
  personName?: string | null
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

  const done = (outcome?: string) =>
    run(() =>
      recordOutcomeAction({
        commandId: randomUUID(),
        personId,
        channel: 'call',
        outcome: outcome ?? 'connected',
        followUpId,
        propertyId,
        dealId,
      }),
    )

  const snooze = (offsetMs: number) =>
    run(() =>
      applyFollowUpCommandAction({
        commandId: randomUUID(),
        commandType: 'snooze',
        payload: { followUpId, snoozeUntil: new Date(Date.now() + offsetMs).toISOString() },
      }),
    )

  const nextAction = (preset: string) =>
    run(() =>
      createQuickNextActionAction({
        commandId: randomUUID(),
        personId,
        propertyId,
        dealId,
        preset,
        source: 'catch_up',
      }),
    )

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      <div className="flex flex-wrap items-center gap-1.5">
        {outcomes.map((o) => (
          <button key={o.code} type="button" disabled={isPending} onClick={() => done(o.code)} className={`${btn} ${btnDone}`}>
            {o.label}
          </button>
        ))}
      </div>
      <div className="flex flex-wrap items-center gap-1.5">
        {snoozePresets.map((s) => (
          <button key={s.label} type="button" disabled={isPending} onClick={() => snooze(s.offsetMs)} className={`${btn} ${btnGhost}`}>
            Snooze · {s.label}
          </button>
        ))}
      </div>
      <button type="button" disabled={isPending} onClick={() => nextAction('call_back')} className={`${btn} ${btnGhost}`}>
        Next: call back
      </button>
      {message && !message.ok ? (
        <span className="text-[10px] font-light text-[var(--portal-archive)]">{message.text}</span>
      ) : null}
    </div>
  )
}
