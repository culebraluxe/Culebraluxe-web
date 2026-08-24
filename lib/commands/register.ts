// ---------------------------------------------------------------------------
// CRM-14J — Canonical command registration.
//
// Wire every thin command wrapper to its stable command type. Adding a new
// business command after this story requires only: define payload/result
// types, implement a thin handler calling an existing canonical domain
// service, register it here, and add targeted tests — no new replay,
// transaction, routing, correlation or audit infrastructure.
// ---------------------------------------------------------------------------

import type { CommandRegistry } from './contracts'
import { InMemoryCommandRegistry } from './registry'
import {
  DEAL_SET_STAGE_UNDER_CONTRACT,
  SetDealStageUnderContractCommand,
} from './deal/set-deal-stage-under-contract'
import {
  DEAL_SET_STAGE_CLOSED,
  SetDealStageClosedCommand,
} from './deal/set-deal-stage-closed'
import {
  DEAL_SET_CLOSING_DATE,
  SetDealClosingDateCommand,
} from './deal/set-deal-closing-date'
import {
  DEAL_SET_FINANCING_TYPE,
  SetDealFinancingTypeCommand,
} from './deal/set-deal-financing-type'
import {
  DEAL_SET_APPRAISAL_REQUIRED,
  SetDealAppraisalRequiredCommand,
} from './deal/set-deal-appraisal-required'
import {
  DEAL_SET_LENDER_CLEAR_TO_CLOSE,
  SetDealLenderClearToCloseCommand,
} from './deal/set-deal-lender-clear-to-close'
import {
  DEAL_SET_INSPECTION_DEADLINE,
  SetDealInspectionDeadlineCommand,
} from './deal/set-deal-inspection-deadline'
import {
  DEAL_SET_FINANCING_DEADLINE,
  SetDealFinancingDeadlineCommand,
} from './deal/set-deal-financing-deadline'
import { OFFER_ACCEPT, AcceptOfferCommand } from './offer/accept-offer'
import {
  TASK_CANCEL,
  TASK_COMPLETE,
  TASK_CREATE,
  CancelTaskCommand,
  CompleteTaskCommand,
  CreateTaskCommand,
} from './task/task-commands'
import {
  INTERACTION_RECORD,
  RecordInteractionCommand,
} from './interaction/record-interaction'
import {
  SIGNATURE_REQUEST_CANCEL,
  SIGNATURE_REQUEST_DECLINE,
  SIGNATURE_REQUEST_SEND,
  SIGNATURE_REQUEST_STATUS,
  CancelSignatureRequestCommand,
  DeclineSignatureRequestCommand,
  SendSignatureRequestCommand,
  StatusSignatureRequestCommand,
} from './signature/signature-commands'
import { DOCUMENT_ISSUE, IssueDocumentCommand } from './document/issue-document'
import {
  AGREEMENT_EXECUTION_CLAIM,
  ClaimAgreementExecutionCommand,
} from './agreement/claim-agreement-execution'

/** Register every canonical command handler into the given registry. */
export function registerCanonicalCommands(registry: CommandRegistry): void {
  registry.register(DEAL_SET_STAGE_UNDER_CONTRACT, new SetDealStageUnderContractCommand())
  registry.register(DEAL_SET_STAGE_CLOSED, new SetDealStageClosedCommand())
  registry.register(DEAL_SET_CLOSING_DATE, new SetDealClosingDateCommand())
  registry.register(DEAL_SET_FINANCING_TYPE, new SetDealFinancingTypeCommand())
  registry.register(DEAL_SET_APPRAISAL_REQUIRED, new SetDealAppraisalRequiredCommand())
  registry.register(DEAL_SET_LENDER_CLEAR_TO_CLOSE, new SetDealLenderClearToCloseCommand())
  // CRM-22 — canonical non-closing milestone deadline commands (XML
  // command-nodes set_inspection_deadline / set_financing_deadline).
  registry.register(DEAL_SET_INSPECTION_DEADLINE, new SetDealInspectionDeadlineCommand())
  registry.register(DEAL_SET_FINANCING_DEADLINE, new SetDealFinancingDeadlineCommand())
  registry.register(OFFER_ACCEPT, new AcceptOfferCommand())
  registry.register(TASK_CREATE, new CreateTaskCommand())
  registry.register(TASK_COMPLETE, new CompleteTaskCommand())
  registry.register(TASK_CANCEL, new CancelTaskCommand())
  // CRM-23 — canonical interaction intake command (integration inbox path).
  registry.register(INTERACTION_RECORD, new RecordInteractionCommand())
  // DOC-03 — provider-neutral signature commands (dispatched by the signature
  // application router through the canonical seam; never per-provider).
  registry.register(SIGNATURE_REQUEST_SEND, new SendSignatureRequestCommand())
  registry.register(SIGNATURE_REQUEST_STATUS, new StatusSignatureRequestCommand())
  registry.register(SIGNATURE_REQUEST_CANCEL, new CancelSignatureRequestCommand())
  registry.register(SIGNATURE_REQUEST_DECLINE, new DeclineSignatureRequestCommand())
  // DOC-06 — canonical issuance command (immutable issued transaction document).
  registry.register(DOCUMENT_ISSUE, new IssueDocumentCommand())
  // CRM-27 — canonical agreement-execution claim command (transactional,
  // exactly-once AGREEMENT_FULLY_EXECUTED emitter through the outbox).
  registry.register(AGREEMENT_EXECUTION_CLAIM, new ClaimAgreementExecutionCommand())
}

/** A fresh registry with every canonical command registered. */
export function createCommandRegistry(): CommandRegistry {
  const registry = new InMemoryCommandRegistry()
  registerCanonicalCommands(registry)
  return registry
}
