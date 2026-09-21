// ---------------------------------------------------------------------------
// Single logical workflow definition seam (Story 137).
//
// The ONE place the application names the logical residential-transaction
// workflow. The start/reconcile/closing-timer paths resolve this logical key
// to the currently-approved deployed version; no business topology is encoded
// here — only the stable logical identity and its pinned version.
// ---------------------------------------------------------------------------

/** Stable logical workflow identity (process_definitions.key). */
export const RESIDENTIAL_TRANSACTION_KEY = 'RE_supermodel'

/** Approved version of the RE_supermodel to start (pinned until re-approved). */
export const RESIDENTIAL_TRANSACTION_VERSION = 1
