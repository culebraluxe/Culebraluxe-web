import type { TemplateDefinition } from "@/lib/forms/template-types"

export type FormSignerCandidate = {
  name: string
  email: string | null
  role: string
  personId: string | null
}

export type FormSignerPerson = {
  personId: string | null
  name: string
  email: string | null
  role: string
}

const SIGNER_EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/

export function formSupportsSigning(template: TemplateDefinition): boolean {
  return template.signatureGroups.length > 0
}

export function isUsableSignerEmail(value: string): boolean {
  return SIGNER_EMAIL_PATTERN.test(value.trim())
}

export function signingStatusLabel(status: string): string {
  switch (status) {
    case "requested":
      return "Requested"
    case "sent":
      return "Sent"
    case "viewed":
      return "Viewed"
    case "signed":
      return "Signed"
    case "completed":
      return "Completed"
    case "declined":
      return "Declined"
    case "voided":
      return "Voided"
    case "expired":
      return "Expired"
    case "error":
      return "Error"
    default:
      return "Sent"
  }
}

export function isActiveSigningStatus(status: string): boolean {
  return (
    status === "requested" ||
    status === "sent" ||
    status === "viewed" ||
    status === "signed"
  )
}

function keyFor(candidate: FormSignerCandidate): string {
  return (
    candidate.personId ||
    candidate.email?.trim().toLowerCase() ||
    candidate.name.trim().toLowerCase()
  )
}

export function pickFormSigners(input: {
  template: TemplateDefinition
  fieldValues: Record<string, string>
  people: FormSignerPerson[]
}): FormSignerCandidate[] {
  const out: FormSignerCandidate[] = []
  const seen = new Set<string>()

  function add(candidate: FormSignerCandidate) {
    const name = candidate.name.trim()
    const email = candidate.email?.trim() || null
    if (!name && !email) return
    const key = keyFor({ ...candidate, name: name || email || "", email })
    if (!key || seen.has(key)) return
    seen.add(key)
    out.push({
      name: name || email || "",
      email,
      role: candidate.role,
      personId: candidate.personId,
    })
  }

  const roles = input.template.signatureGroups.map((group) => group.role)
  for (const role of roles) {
    for (const person of input.people) {
      if (person.role === role) add(person)
    }
  }
  for (const person of input.people) add(person)
  for (const group of input.template.signatureGroups) {
    if (!group.field) continue
    const name = (input.fieldValues[group.field] ?? "").trim()
    if (name) {
      add({
        name,
        email: null,
        role: group.role,
        personId: null,
      })
    }
  }
  return out
}
