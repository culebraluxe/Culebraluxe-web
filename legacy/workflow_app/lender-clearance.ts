// Pure mapping from the canonical deal.lender_clear_to_close fact to the
// workflow decision input. Kept db-free so it can be tested without a database.
//
// Lender clear-to-close (CRM-20) is application-owned canonical truth, never
// lender provider behavior: true = the lender cleared the transaction to
// close, false = the lender has not cleared, NULL = unresolved (financed:
// not yet recorded; cash: not applicable). Only financed deals consume the
// fact — the closing-readiness gate routes cash/non-financed deals around it
// entirely. NULL is preserved so the XML decision can surface it explicitly
// instead of silently treating the deal as closing-ready.

export function lenderClearToCloseFromFact(value: boolean | null): boolean | null {
  if (value === true) return true
  if (value === false) return false
  return null
}
