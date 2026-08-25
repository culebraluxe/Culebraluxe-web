// ---------------------------------------------------------------------------
// CORE-DAILY-02 — reusable, context-aware contact targets (PURE).
//
// Builds safe native launch targets (tel / mailto / sms / WhatsApp) from
// verified contact evidence. Launching a native URL NEVER records a successful
// communication (that is a separate outcome-capture command). Unavailable
// channels are omitted honestly; multiple legitimate contact points are offered
// as a chooser. No communications client, no automatic send, no private values
// in logs, no arbitrary URL construction.
// ---------------------------------------------------------------------------

export type ContactChannel = 'call' | 'email' | 'sms' | 'whatsapp'

export type ContactTarget = {
  channel: ContactChannel
  /** human label, e.g. 'Work · +1 (787) 555-0134' */
  label: string
  /** safe native URL */
  url: string
  /** value actually launched (for UI display) */
  display: string
}

/** US/Puerto Rico phone: 10 digits, or 11 digits with leading country '1'. */
export function normalizeUsPhone(input: string): string | null {
  const digits = input.replace(/\D+/g, '')
  if (digits.length === 10) return digits
  if (digits.length === 11 && digits.startsWith('1')) return digits.slice(1)
  return null
}

/** RFC-simple email validation for safe mailto construction. */
export function isValidEmail(input: string): boolean {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(input.trim())
}

/** Safe tel: URL — only US/PR digits (E.164). Null when unreliable. */
export function buildCallTarget(rawPhone: string, label?: string | null): ContactTarget | null {
  const p = normalizeUsPhone(rawPhone)
  if (!p) return null
  return {
    channel: 'call',
    label: label ? `${label} · +1 ${p}` : `+1 ${p}`,
    url: `tel:+1${p}`,
    display: `+1 ${p}`,
  }
}

/** Safe sms: URL — only US/PR digits. Null when unreliable. */
export function buildSmsTarget(rawPhone: string, label?: string | null): ContactTarget | null {
  const p = normalizeUsPhone(rawPhone)
  if (!p) return null
  return {
    channel: 'sms',
    label: label ? `${label} · +1 ${p}` : `+1 ${p}`,
    url: `sms:+1${p}`,
    display: `+1 ${p}`,
  }
}

/** Safe mailto: URL — validated email, encodeURIComponent of the address. */
export function buildEmailTarget(rawEmail: string, label?: string | null): ContactTarget | null {
  const email = rawEmail.trim()
  if (!isValidEmail(email)) return null
  return {
    channel: 'email',
    label: label ? `${label} · ${email}` : email,
    url: `mailto:${encodeURIComponent(email)}`,
    display: email,
  }
}

/**
 * Safe WhatsApp launch. `approved` is an explicit product gate (WhatsApp is only
 * launched when an eligible/approved path exists); we never guess international
 * numbers. `text` is optional pre-filled plain message (URL-encoded, no HTML).
 */
export function buildWhatsAppTarget(
  rawPhone: string,
  opts?: { approved?: boolean; text?: string; label?: string | null },
): ContactTarget | null {
  if (opts?.approved !== true) return null
  const p = normalizeUsPhone(rawPhone)
  if (!p) return null
  const params = new URLSearchParams()
  if (opts.text?.trim()) params.set('text', opts.text.trim())
  const qs = params.toString()
  return {
    channel: 'whatsapp',
    label: opts?.label ? `${opts.label} · WhatsApp` : `WhatsApp · +1 ${p}`,
    url: `https://wa.me/1${p}${qs ? `?${qs}` : ''}`,
    display: `+1 ${p}`,
  }
}

/** Input evidence: distinct email + phone values (normalized) for one person. */
export type ContactEvidence = {
  emails: string[]
  phones: string[]
  emailLabels?: Record<string, string | null>
  phoneLabels?: Record<string, string | null>
  whatsappApproved?: boolean
}

/**
 * Resolve the available, honest contact targets for a person.
 * - dedupes values
 * - omits unavailable/unreliable channels
 * - never invents a number or opens arbitrary URLs
 * The returned array is ordered Call → Message → Email → WhatsApp.
 */
export function resolveContactTargets(evidence: ContactEvidence): ContactTarget[] {
  const seen = new Set<string>()
  const out: ContactTarget[] = []

  const phones = Array.from(new Set(evidence.phones.filter(Boolean)))
  for (const phone of phones) {
    const label = evidence.phoneLabels?.[phone] ?? null
    const call = buildCallTarget(phone, label)
    if (call && !seen.has(call.url)) { seen.add(call.url); out.push(call) }
  }
  for (const phone of phones) {
    const label = evidence.phoneLabels?.[phone] ?? null
    const sms = buildSmsTarget(phone, label)
    if (sms && !seen.has(sms.url)) { seen.add(sms.url); out.push(sms) }
  }
  const emails = Array.from(new Set(evidence.emails.filter(Boolean)))
  for (const email of emails) {
    const label = evidence.emailLabels?.[email] ?? null
    const mail = buildEmailTarget(email, label)
    if (mail && !seen.has(mail.url)) { seen.add(mail.url); out.push(mail) }
  }
  if (evidence.whatsappApproved) {
    for (const phone of phones) {
      const label = evidence.phoneLabels?.[phone] ?? null
      const wa = buildWhatsAppTarget(phone, { approved: true, label })
      if (wa && !seen.has(wa.url)) { seen.add(wa.url); out.push(wa) }
    }
  }

  return out
}
