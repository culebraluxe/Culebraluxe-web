// ---------------------------------------------------------------------------
// INTAKE-01 — Canonical intake message contract (public seam).
//
// One normalized intake message; two acquisition lanes (realtime + batch);
// one transformation stack. Every edge lowers into CanonicalIntakeMessage;
// the durable inbox projection (toInboxInsert) is the single bridge into the
// existing integration inbox, identity resolution and Business Command layer.
// ---------------------------------------------------------------------------

export type {
  CanonicalIntakeMessage,
  IntakeAcquisitionLane,
  IntakeAttachment,
  IntakeContent,
  IntakeContext,
  IntakeIdentity,
  IntakeIdentityKind,
  IntakeParticipant,
  IntakeParticipantRole,
  IntakeProvenance,
  IntakeSource,
  IntakeThread,
} from './contracts'
export {
  INTAKE_MESSAGE_SCHEMA_VERSION,
  INTAKE_SOURCE_PAYLOAD_MAX_BYTES,
  assertValidIntakeMessage,
  intakeDedupeKey,
  intakeSourceIdentity,
  validateIntakeMessage,
} from './contracts'
export {
  lowerExternalActivityEventToIntakeMessage,
} from './realtime'
export type {
  IntakeBatchItemInput,
  IntakeBatchManifest,
} from './batch'
export {
  lowerBatchItemToIntakeMessage,
} from './batch'
export {
  toInboxInsert,
} from './inbox'
