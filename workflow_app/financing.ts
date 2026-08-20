// Pure mapping from the canonical deal.financing_type fact to the workflow
// decision input. Kept db-free so it can be tested without a database.

export function financingApplicableFromType(
  financingType: string | null,
): boolean | null {
  if (financingType === 'financed') return true
  if (financingType === 'cash') return false
  return null
}
