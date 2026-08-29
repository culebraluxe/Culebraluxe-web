// ---------------------------------------------------------------------------
// FORMS TEMPLATE VERSIONING — structural compatibility validator.
//
// A content-only legal revision may change wording but must NOT redefine the
// locked document/execution contract. compareTemplateStructure(prev, next)
// detects changes to STRUCTURAL identity only:
//   - participant role declarations + multiplicity
//   - canonical field ids + structurally significant types + bindings
//   - signature-group roles, field binding, initials requirement
//
// Legal prose (section default text) is EXPECTED to change and is reported as
// `contentChanged` but never fails compatibility. This is a PURE, deterministic
// checker — it is NOT an AI feature and it never edits or "fixes" a template.
// ---------------------------------------------------------------------------

import type { TemplateDefinition } from './template-types'

export type StructureStatus = 'unchanged' | string[]

export type TemplateStructuralReport = {
  /** Whether any legal prose (section default text) differs. Allowed. */
  contentChanged: boolean
  /** Participant role changes (role codes affected). */
  participants: StructureStatus
  /** Canonical field / binding / type changes (field ids affected). */
  canonicalFields: StructureStatus
  /** Signature-group changes (role codes affected). */
  signatureGroups: StructureStatus
  /** Aggregate structural status (participants + fields + signature groups). */
  executionStructure: StructureStatus
  /** True when structure is unchanged (prose may differ freely). */
  compatible: boolean
}

/** Section prose is identical when every segment matches. */
function sectionsEqual(a: TemplateDefinition, b: TemplateDefinition): boolean {
  if (a.sections.length !== b.sections.length) return false
  for (let i = 0; i < a.sections.length; i++) {
    const sa = a.sections[i]
    const sb = b.sections.find((x) => x.name === sa.name)
    if (!sb) return false
    if (sa.segments.length !== sb.segments.length) return false
    for (let j = 0; j < sa.segments.length; j++) {
      const x = sa.segments[j]
      const y = sb.segments[j]
      if (x.kind !== y.kind) return false
      if (x.kind === 'text' && y.kind === 'text' && x.text !== y.text) return false
      if (x.kind === 'value' && y.kind === 'value' && x.field !== y.field) return false
    }
  }
  return true
}

function participantChanges(a: TemplateDefinition, b: TemplateDefinition): string[] {
  const changes: string[] = []
  const roles = new Set([
    ...a.participants.map((p) => p.role),
    ...b.participants.map((p) => p.role),
  ])
  for (const role of roles) {
    const pa = a.participants.find((p) => p.role === role)
    const pb = b.participants.find((p) => p.role === role)
    if (!pa || !pb) {
      changes.push(role)
      continue
    }
    if (pa.multiple !== pb.multiple) changes.push(role)
  }
  return changes
}

function fieldChanges(a: TemplateDefinition, b: TemplateDefinition): string[] {
  const changes: string[] = []
  const names = new Set([
    ...a.fields.map((f) => f.name),
    ...b.fields.map((f) => f.name),
  ])
  for (const name of names) {
    const fa = a.fields.find((f) => f.name === name)
    const fb = b.fields.find((f) => f.name === name)
    if (!fa || !fb) {
      changes.push(name)
      continue
    }
    // Structurally significant: type + canonical-data binding.
    if (fa.type !== fb.type) changes.push(name)
    if ((fa.binding ?? null) !== (fb.binding ?? null)) changes.push(name)
  }
  return changes
}

function signatureGroupChanges(a: TemplateDefinition, b: TemplateDefinition): string[] {
  const changes: string[] = []
  const roles = new Set([
    ...a.signatureGroups.map((g) => g.role),
    ...b.signatureGroups.map((g) => g.role),
  ])
  for (const role of roles) {
    const ga = a.signatureGroups.find((g) => g.role === role)
    const gb = b.signatureGroups.find((g) => g.role === role)
    if (!ga || !gb) {
      changes.push(role)
      continue
    }
    if ((ga.field ?? null) !== (gb.field ?? null)) changes.push(role)
    if (ga.initials !== gb.initials) changes.push(role)
  }
  return changes
}

/**
 * Compare the previous approved version against a proposed new version.
 * Returns a readable report: which structural seams changed (with the affected
 * participant/field/signature identities) and whether the revision is
 * structurally compatible. Legal prose differences are allowed and reported
 * separately as `contentChanged`.
 */
export function compareTemplateStructure(
  prev: TemplateDefinition,
  next: TemplateDefinition,
): TemplateStructuralReport {
  const participants = participantChanges(prev, next)
  const canonicalFields = fieldChanges(prev, next)
  const signatureGroups = signatureGroupChanges(prev, next)
  const contentChanged = !sectionsEqual(prev, next)

  const executionParts = [
    ...participants.map((x) => `participant:${x}`),
    ...canonicalFields.map((x) => `field:${x}`),
    ...signatureGroups.map((x) => `signature:${x}`),
  ]
  const executionStructure: StructureStatus =
    executionParts.length === 0 ? 'unchanged' : executionParts

  const compatible =
    participants.length === 0 && canonicalFields.length === 0 && signatureGroups.length === 0

  return {
    contentChanged,
    participants: participants.length === 0 ? 'unchanged' : participants,
    canonicalFields: canonicalFields.length === 0 ? 'unchanged' : canonicalFields,
    signatureGroups: signatureGroups.length === 0 ? 'unchanged' : signatureGroups,
    executionStructure,
    compatible,
  }
}
