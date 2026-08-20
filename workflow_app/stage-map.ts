// Deal-stage ownership for CRM-14.
//
// CRM-14 workflow becomes the owner of consequential deal progression, but the
// two notions stay distinct:
//
//   workflow state  = orchestration state (the engine's process/token model)
//   deal stage      = canonical business state (deal.stage)
//
// The workflow only changes deal.stage through explicit application commands
// (`deal.set_stage_under_contract`, `deal.set_stage_closed`). Everything else
// is orchestration state that does not mutate business rows.

import type { DealStage } from '../lib/portal/types'

export type StageChange = {
  milestoneId: string
  from: DealStage
  to: DealStage
}

export const DEAL_STAGE_CHANGES: StageChange[] = [
  {
    milestoneId: 'mark_under_contract',
    from: 'offer',
    to: 'under_contract',
  },
  {
    milestoneId: 'mark_closed',
    from: 'under_contract',
    to: 'closed',
  },
]

export function stageChangeFor(milestoneId: string): StageChange | null {
  return DEAL_STAGE_CHANGES.find((c) => c.milestoneId === milestoneId) ?? null
}
