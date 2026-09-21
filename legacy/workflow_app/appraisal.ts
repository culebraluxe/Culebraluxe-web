// Pure mapping from the canonical deal.appraisal_required fact to the workflow
// decision input. Kept db-free so it can be tested without a database.
//
// Appraisal applicability is independent of financing (Story 123): the
// canonical fact is set by explicit human/application resolution, never
// derived from financing_type. NULL (unresolved) is preserved so the XML
// decision can surface it explicitly instead of silently skipping.

export function appraisalApplicableFromRequired(
  appraisalRequired: boolean | null,
): boolean | null {
  if (appraisalRequired === true) return true
  if (appraisalRequired === false) return false
  return null
}
