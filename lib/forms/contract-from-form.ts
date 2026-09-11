import type { TemplateDefinition, TemplateFieldBinding, TemplateFieldValues } from './template-types'

/**
 * The BRIDGE: a form's declared shape -> a Contract.
 *
 * A template already declares the whole contract shape, which is why this is a
 * pure function with no database and no domain knowledge of its own:
 *
 *   <participants role="...">          -> the contract ROLES (the one vocabulary)
 *   <signature-group role field="..."> -> role -> its NAME field (snapshot_name)
 *   <field ... binding="...">          -> a LINK resolved from canon (never a fact)
 *   <field ...> (unbound)              -> a contract FACT (contract.facts)
 *   the subject property               -> contract_property.SUBJECT_PROPERTY
 *
 * Nothing here invents a field, a role, or a value. If the template did not
 * declare it, it is not part of the contract.
 */

/** One resolved party for a participant role. */
export type ResolvedParty = {
  personId?: string | null
  firmId?: string | null
  /** Overrides the name field value when the operator picked a different person. */
  snapshotName?: string | null
}

export type ContractFromFormInput = {
  template: TemplateDefinition
  /** The form instance's entered values. */
  values: TemplateFieldValues
  /** The contract type this template produces (e.g. 'listing_agreement'). */
  contractType: string
  /** The new contract's id (uuid), supplied by the caller. */
  contractId: string
  /** The subject property (bucket 1). Null = this template has no property. */
  propertyId?: string | null
  /** The chosen party per participant role, from the form's selections. */
  parties?: Readonly<Record<string, readonly ResolvedParty[]>>
  /** The contract this one continues (a P&S joins the listing chain and the offer chain). */
  predecessorContractIds?: readonly string[]
  /** Provenance: which form instance produced it. */
  sourceFormInstanceId?: string | null
}

export type ContractRolePlan = {
  role: string
  label: string
  kind: 'person' | 'firm'
  identityId: string | null
  /** The party's name as it will be frozen on the contract. */
  snapshotName: string | null
  ordinal: number
  /** The template field this name came from. */
  nameField: string | null
  /** True when the template allows more than one party in this role. */
  multiple: boolean
}

export type ContractFromFormPlan = {
  contractType: string
  formTemplateId: string
  templateVersion: number
  propertyId: string | null
  predecessorContractIds: readonly string[]
  sourceFormInstanceId: string | null
  roles: ContractRolePlan[]
  facts: Record<string, string>
  /** Fields the template binds to canonical data: resolved, never stored as facts. */
  linkedFields: Array<{ field: string; binding: TemplateFieldBinding }>
  /** Template fields consumed as a role's name (so they are not "facts"). */
  roleNameFields: Array<{ field: string; role: string }>
  /** Participant roles the template declares that have no party selected yet. */
  missingParties: string[]
}


/** `buyerName` is the conventional name field for role `BUYER`, used when the
 *  template does not declare a signature-group mapping for that role. */
function conventionalNameField(role: string): string | null {
  const camel = role
    .toLowerCase()
    .split('_')
    .map((part, index) => (index === 0 ? part : part.charAt(0).toUpperCase() + part.slice(1)))
    .join('')
  return `${camel}Name`
}

/**
 * Build the contract a form describes. Pure: same inputs, same plan, no I/O.
 * The plan is what `contract.createFromForm` needs, plus the reasoning for why
 * each field landed where it did.
 */
export function planContractFromForm(input: ContractFromFormInput): ContractFromFormPlan {
  const { template, values } = input

  const signatureFieldByRole = new Map<string, string>()
  for (const group of template.signatureGroups) {
    if (group.field) signatureFieldByRole.set(group.role, group.field)
  }

  const roles: ContractRolePlan[] = []
  const roleNameFields: Array<{ field: string; role: string }> = []
  const missingParties: string[] = []

  for (const participant of template.participants) {
    const nameField =
      signatureFieldByRole.get(participant.role) ?? conventionalNameField(participant.role)
    if (nameField && template.fields.some((field) => field.name === nameField)) {
      roleNameFields.push({ field: nameField, role: participant.role })
    }

    const selected = input.parties?.[participant.role] ?? []
    if (selected.length === 0) {
      missingParties.push(participant.role)
      // The role still exists on the contract: an unfilled role is visible work,
      // not a silent omission.
      roles.push({
        role: participant.role,
        label: participant.label,
        kind: 'person',
        identityId: null,
        snapshotName: nameField ? values[nameField]?.trim() || null : null,
        ordinal: 0,
        nameField,
        multiple: participant.multiple,
      })
      continue
    }

    selected.forEach((party, index) => {
      const kind: 'person' | 'firm' = party.firmId ? 'firm' : 'person'
      roles.push({
        role: participant.role,
        label: participant.label,
        kind,
        identityId: (kind === 'firm' ? party.firmId : party.personId) ?? null,
        snapshotName:
          party.snapshotName?.trim() || (nameField ? values[nameField]?.trim() || null : null),
        ordinal: index,
        nameField,
        multiple: participant.multiple,
      })
    })
  }

  // Facts = declared fields that are NEITHER a canonical link NOR a role's name.
  const consumedNameFields = new Set(roleNameFields.map((entry) => entry.field))
  const linkedFields: Array<{ field: string; binding: TemplateFieldBinding }> = []
  const facts: Record<string, string> = {}

  for (const field of template.fields) {
    if (field.binding) {
      linkedFields.push({ field: field.name, binding: field.binding })
      continue
    }
    if (consumedNameFields.has(field.name)) continue
    const value = values[field.name]
    if (typeof value === 'string' && value.trim() !== '') facts[field.name] = value
  }

  return {
    contractType: input.contractType,
    formTemplateId: template.id,
    templateVersion: template.version,
    propertyId: input.propertyId ?? null,
    predecessorContractIds: input.predecessorContractIds ?? [],
    sourceFormInstanceId: input.sourceFormInstanceId ?? null,
    roles,
    facts,
    linkedFields,
    roleNameFields,
    missingParties,
  }
}
