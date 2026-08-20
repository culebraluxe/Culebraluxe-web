// ---------------------------------------------------------------------------
// CulebraLuxe workflow configuration defaults (Story 134, class B facts).
//
// These are the operating defaults for Culebra, Puerto Rico — the current
// production jurisdiction. They are APPLICATION configuration, never engine
// behavior: the RE_supermodel XML remains jurisdiction-neutral and the engine
// never sees any jurisdiction concept. A future Florida (or other) operating
// pattern supplies a different configuration for the SAME model.
//
// Fact classification (Story 134):
//   A — derived from canonical application data (deal.financing_type, deal.closing_date)
//   B — configuration/default appropriate for CulebraLuxe (below)
//   C — unresolved; requires human/application resolution (never invented)
//   D — not yet necessary for V1 (none)
// ---------------------------------------------------------------------------

export type JurisdictionConfig = {
  /** Closing professional for the jurisdiction. */
  closingAgentRole: 'notario' | 'title_company' | 'attorney'
  requiresNotario: boolean
  requiresTitleCompany: boolean
  /** PR: tax/municipal clearance includes CRIM. */
  requiresCrimClearance: boolean
  /** Post-closing registry/recording follow-up. */
  requiresRegistryFollowup: boolean
  /** Residential inspections apply by default in Culebra. */
  inspectionApplicable: boolean
  /** Hazard/property insurance applies by default in Culebra. */
  insuranceApplicable: boolean
  /** Survey: not required unless land/boundary concerns. */
  requiresSurvey: boolean
  /** HOA/condo clearance: not required unless the property is a condo. */
  requiresHoaClearance: boolean
  /** Final brokerage confirmation before closing (Story 136: "if desired"). */
  closingConfirmationRequired: boolean
}

/** Culebra, Puerto Rico operating defaults (class B). */
export const CULEBRA_JURISDICTION_CONFIG: JurisdictionConfig = {
  closingAgentRole: 'notario',
  requiresNotario: true,
  requiresTitleCompany: false,
  requiresCrimClearance: true,
  requiresRegistryFollowup: true,
  inspectionApplicable: true,
  insuranceApplicable: true,
  requiresSurvey: false,
  requiresHoaClearance: false,
  closingConfirmationRequired: true,
}
