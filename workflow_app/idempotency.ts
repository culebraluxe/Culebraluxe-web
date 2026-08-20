// Application command-idempotency seam.
//
// The canonical claim-first receipt helpers live in the domain layer
// (db/workflow-command-receipt.ts) so the claim can participate in the same
// transaction as the business effect. Re-exported here for application-layer
// callers.
export {
  claimReceipt,
  finalizeReceipt,
  readFinalReceipt,
  type CommandReceipt,
} from '../db/workflow-command-receipt'
