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
import { OFFER_ACCEPT, AcceptOfferCommand } from './offer/accept-offer'
import {
  TASK_CANCEL,
  TASK_COMPLETE,
  TASK_CREATE,
  CancelTaskCommand,
  CompleteTaskCommand,
  CreateTaskCommand,
} from './task/task-commands'

/** Register every canonical command handler into the given registry. */
export function registerCanonicalCommands(registry: CommandRegistry): void {
  registry.register(DEAL_SET_STAGE_UNDER_CONTRACT, new SetDealStageUnderContractCommand())
  registry.register(DEAL_SET_STAGE_CLOSED, new SetDealStageClosedCommand())
  registry.register(DEAL_SET_CLOSING_DATE, new SetDealClosingDateCommand())
  registry.register(DEAL_SET_FINANCING_TYPE, new SetDealFinancingTypeCommand())
  registry.register(DEAL_SET_APPRAISAL_REQUIRED, new SetDealAppraisalRequiredCommand())
  registry.register(DEAL_SET_LENDER_CLEAR_TO_CLOSE, new SetDealLenderClearToCloseCommand())
  registry.register(OFFER_ACCEPT, new AcceptOfferCommand())
  registry.register(TASK_CREATE, new CreateTaskCommand())
  registry.register(TASK_COMPLETE, new CompleteTaskCommand())
  registry.register(TASK_CANCEL, new CancelTaskCommand())
}

/** A fresh registry with every canonical command registered. */
export function createCommandRegistry(): CommandRegistry {
  const registry = new InMemoryCommandRegistry()
  registerCanonicalCommands(registry)
  return registry
}
