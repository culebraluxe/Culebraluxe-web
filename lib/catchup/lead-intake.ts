// ---------------------------------------------------------------------------
// CATCH-UP — website lead intake normalization (PURE).
//
// Minimal V1 intake: Name + (Email OR Phone). At least one contact method is
// required; a long questionnaire is never required. Budget / financing /
// timeline / neighborhood / property type / tags / campaign / stage / score are
// NOT required. Those facts emerge naturally through conversation and Ara.
// ---------------------------------------------------------------------------

export type NormalizedLead = {
  name: string
  email?: string
  phone?: string
  message?: string
}

export type LeadNormalizationResult =
  | { ok: true; value: NormalizedLead }
  | { ok: false; errors: Partial<Record<'name' | 'email' | 'phone', string>> }

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/

export function normalizeLeadInput(raw: {
  name?: unknown
  email?: unknown
  phone?: unknown
  message?: unknown
}): LeadNormalizationResult {
  const errors: Partial<Record<'name' | 'email' | 'phone', string>> = {}

  const name = typeof raw.name === 'string' ? raw.name.trim() : ''
  if (!name) errors.name = 'Name is required.'

  const email = typeof raw.email === 'string' ? raw.email.trim() : ''
  const phone =
    typeof raw.phone === 'string'
      ? raw.phone.replace(/[^+\d]/g, '')
      : ''

  if (!email && !phone) {
    errors.email = 'Add an email or phone.'
  }
  if (email && !EMAIL_RE.test(email)) {
    errors.email = 'Enter a valid email.'
  }

  if (Object.keys(errors).length > 0) {
    return { ok: false, errors }
  }

  const message =
    typeof raw.message === 'string' && raw.message.trim()
      ? raw.message.trim()
      : undefined

  return {
    ok: true,
    value: {
      name,
      ...(email ? { email } : {}),
      ...(phone ? { phone } : {}),
      ...(message ? { message } : {}),
    },
  }
}
