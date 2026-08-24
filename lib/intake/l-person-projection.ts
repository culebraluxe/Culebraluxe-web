// ---------------------------------------------------------------------------
// SUPPORT-2 — Apple Contacts staged profile -> l_person relational-load mapping.
//
// Pure, deterministic mapper from one IMMUTABLE staged-contact revision
// (integration_staged_contact_profile.profile JSONB) into:
//   - one l_person current-state load row (flattened, searchable)
//   - its labeled l_person_identity children (emails / phones / apple_contact)
//   - its l_person_address children (postal addresses, preserved separately)
//
// The staged profile JSON contract (produced by scripts/load-apple-contacts.ts
// normalizeProfile):
//   {
//     name:  { prefix, given, middle, family, suffix, nickname },
//     organization, department, jobTitle: string,
//     emails:  [{ label, value }],
//     phones:  [{ label, value }],
//     postalAddresses: [{ label, street, city, state, postalCode, country,
//                        isoCountryCode }]
//   }
//
// Rules:
//   - Never invents buyer/seller role, budget, preferences, timeline, or any
//     canonical business fact absent from Apple Contacts.
//   - Missing optional fields map to null/empty, never fabricated.
//   - Multiple labeled emails/phones are preserved (never collapsed).
//   - A primary/preferred indicator is NOT invented (Apple does not establish one).
//   - Postal addresses are NOT identities in the canonical model, so they are
//     projected into l_person_address, never misclassified into l_person_identity.
//   - Deterministic + deduplicated: the same staged revision always produces the
//     same set of identities (replay-safe).
// ---------------------------------------------------------------------------

export type LPersonProjection = {
  displayName: string
  namePrefix: string | null
  givenName: string | null
  middleName: string | null
  familyName: string | null
  nameSuffix: string | null
  nickname: string | null
  organization: string | null
  department: string | null
  jobTitle: string | null
  displayAddress: string | null
}

export type LPersonIdentityProjection = {
  identityType: 'email' | 'phone' | 'apple_contact'
  identityValue: string
  originalValue: string
  normalizedValue: string
  sourceLabel: string | null
  sourceSystem: string
  isPrimary: boolean
  ordinal: number
}

export type LPersonAddressProjection = {
  sourceLabel: string | null
  street: string
  city: string
  state: string
  postalCode: string
  country: string
  isoCountryCode: string
  ordinal: number
}

export type StagedContactInput = {
  stagedProfileId: string
  intakeBatchId: string | null
  source: string
  sourceAccount: string
  sourceContactId: string
  revision: number
  payloadFingerprint: string
  reconciliationStatus: string
  candidatePersonId: string | null
  /** The immutable staged profile JSON (normalizeProfile shape). */
  profile: Record<string, unknown>
}

export type LPersonProjectionResult = {
  lPerson: LPersonProjection
  identities: LPersonIdentityProjection[]
  addresses: LPersonAddressProjection[]
}

function s(value: unknown): string {
  return typeof value === 'string' ? value : ''
}

function nullable(value: unknown): string | null {
  const text = s(value).trim()
  return text === '' ? null : text
}

function nameParts(profile: Record<string, unknown>): string[] {
  const name =
    profile.name && typeof profile.name === 'object'
      ? (profile.name as Record<string, unknown>)
      : {}
  return ['prefix', 'given', 'middle', 'family', 'suffix']
    .map((k) => s(name[k]).trim())
    .filter(Boolean)
}

function normalizePhone(value: string): string {
  // E.164-style canonical key for the load projection: always `+<digits>` so
  // a number with and without a leading '+' dedup to the same identity.
  const trimmed = value.trim()
  const digits = trimmed.replace(/\D/g, '')
  return `+${digits}`
}

function normalizeEmail(value: string): string {
  return value.trim().toLowerCase()
}

function arrayOf(value: unknown): Record<string, unknown>[] {
  return Array.isArray(value) ? (value as Record<string, unknown>[]) : []
}

function displayAddressOf(profile: Record<string, unknown>): string | null {
  const first = arrayOf(profile['postalAddresses'])[0]
  if (!first) return null
  const street = s(first['street']).trim()
  const city = s(first['city']).trim()
  const state = s(first['state']).trim()
  const postalCode = s(first['postalCode']).trim()
  const country = s(first['country']).trim()
  // "Culebra, PR 00775" (state + postal code together, no comma between).
  const location = [city, [state, postalCode].filter(Boolean).join(' ')]
    .filter(Boolean)
    .join(', ')
  const parts = [street, location || null, country || null].filter(Boolean)
  return parts.length > 0 ? parts.join(', ') : null
}


/** Map one immutable staged revision to its l_person load projection. Pure. */
export function projectLPersonFromStaged(
  input: StagedContactInput,
): LPersonProjectionResult {
  const profile = input.profile ?? {}

  const personalName = nameParts(profile).join(' ')
  const organization = nullable(profile['organization'])
  const nameObj =
    profile.name && typeof profile.name === 'object'
      ? (profile.name as Record<string, unknown>)
      : {}
  const nickname = nullable(nameObj['nickname'])
  const displayName =
    personalName ||
    organization ||
    nickname ||
    input.sourceContactId.trim() ||
    '(unnamed)'

  const lPerson: LPersonProjection = {
    displayName,
    namePrefix: nullable(nameObj['prefix']),
    givenName: nullable(nameObj['given']),
    middleName: nullable(nameObj['middle']),
    familyName: nullable(nameObj['family']),
    nameSuffix: nullable(nameObj['suffix']),
    nickname,
    organization,
    department: nullable(profile['department']),
    jobTitle: nullable(profile['jobTitle']),
    displayAddress: displayAddressOf(profile),
  }

  const identities: LPersonIdentityProjection[] = []
  const seen = new Set<string>()
  const addIdentity = (identity: LPersonIdentityProjection) => {
    if (!identity.identityValue) return
    const key = `${identity.identityType}|${identity.identityValue.toLowerCase()}`
    if (seen.has(key)) return
    seen.add(key)
    identities.push(identity)
  }

  // Emails (multiple labeled emails preserved, never collapsed).
  arrayOf(profile['emails']).forEach((email, index) => {
    const raw = s(email['value']).trim()
    if (!raw) return
    const normalized = normalizeEmail(raw)
    addIdentity({
      identityType: 'email',
      identityValue: normalized,
      originalValue: raw,
      normalizedValue: normalized,
      sourceLabel: nullable(email['label']),
      sourceSystem: input.source,
      isPrimary: false,
      ordinal: index,
    })
  })

  // Phones (multiple labeled phones preserved, never collapsed).
  arrayOf(profile['phones']).forEach((phone, index) => {
    const raw = s(phone['value']).trim()
    if (!raw) return
    const normalized = normalizePhone(raw)
    addIdentity({
      identityType: 'phone',
      identityValue: normalized,
      originalValue: raw,
      normalizedValue: normalized,
      sourceLabel: nullable(phone['label']),
      sourceSystem: input.source,
      isPrimary: false,
      ordinal: index,
    })
  })

  // Apple source/contact identifier (consistent with the canonical person_identity
  // 'apple_contact' identity_type convention).
  const appleId = input.sourceContactId.trim()
  if (appleId) {
    addIdentity({
      identityType: 'apple_contact',
      identityValue: appleId,
      originalValue: appleId,
      normalizedValue: appleId,
      sourceLabel: null,
      sourceSystem: input.source,
      isPrimary: false,
      ordinal: 0,
    })
  }

  // Postal addresses (own child projection; not identities).
  const addresses: LPersonAddressProjection[] = arrayOf(profile['postalAddresses']).map(
    (addr, index) => ({
      sourceLabel: nullable(addr['label']),
      street: s(addr['street']).trim(),
      city: s(addr['city']).trim(),
      state: s(addr['state']).trim(),
      postalCode: s(addr['postalCode']).trim(),
      country: s(addr['country']).trim(),
      isoCountryCode: s(addr['isoCountryCode']).trim(),
      ordinal: index,
    }),
  )

  return { lPerson, identities, addresses }
}
