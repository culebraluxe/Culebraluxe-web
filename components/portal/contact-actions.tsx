'use client'

import Link from 'next/link'

import { resolveContactTargets, type ContactEvidence } from '@/lib/relationship-intel/contact-targets'

// ---------------------------------------------------------------------------
// CORE-DAILY-02 — reusable, context-aware native contact actions.
// Launching a native URL NEVER records successful communication (outcome
// capture is a separate command). Unavailable channels are omitted honestly.
// ---------------------------------------------------------------------------

const channelGlyph: Record<string, string> = {
  call: 'Call',
  email: 'Email',
  sms: 'Message',
  whatsapp: 'WhatsApp',
}

const buttonClass =
  'inline-flex min-h-11 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-border)] px-3 text-[11px] font-light uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)]'

export function ContactActions({
  evidence,
  personName,
  onLaunched,
}: {
  evidence: ContactEvidence
  personName?: string | null
  onLaunched?: () => void
}) {
  const targets = resolveContactTargets(evidence)
  if (targets.length === 0) {
    return (
      <span className="text-[11px] font-light text-black/35">
        No contact method available
      </span>
    )
  }
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {targets.map((t) => (
        <a
          key={t.channel + t.url}
          href={t.url}
          onClick={() => onLaunched?.()}
          aria-label={`${channelGlyph[t.channel] ?? t.channel} ${personName ?? ''} — ${t.display}`}
          className={buttonClass}
        >
          {channelGlyph[t.channel] ?? t.channel}
        </a>
      ))}
    </div>
  )
}
