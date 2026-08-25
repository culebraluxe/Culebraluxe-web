// ---------------------------------------------------------------------------
// CORE-DAILY-13 — privacy-conscious daily-loop product telemetry.
//
// Emits a minimal, non-identifying event row (event_type + stable internal ids
// + a tiny metadata object). NEVER stores message content, email bodies,
// sensitive notes, or contact values. Telemetry never blocks the user action
// and there is no vanity dashboard.
// ---------------------------------------------------------------------------
import { sql } from './client'
import type { QueryExecutor } from './query-executor'

export type DailyLoopTelemetryEvent =
  | 'catch_up_opened'
  | 'client_opened'
  | 'contract_opened'
  | 'contact_action_invoked'
  | 'outcome_recorded'
  | 'followup_completed'
  | 'followup_snoozed'
  | 'next_touch_created'
  | 'recommendation_dismissed'

export async function emitDailyLoopTelemetry(input: {
  eventType: DailyLoopTelemetryEvent
  entityKind?: string | null
  entityId?: string | null
  metadata?: Record<string, unknown>
  execute?: QueryExecutor
}): Promise<void> {
  const execute = input.execute ?? sql
  // Best-effort: telemetry must never block or fail the user's action.
  try {
    await execute`
      insert into daily_loop_telemetry (event_type, entity_kind, entity_id, metadata)
      values (
        ${input.eventType},
        ${input.entityKind ?? null},
        ${input.entityId ?? null},
        ${JSON.stringify(input.metadata ?? {})}::jsonb
      )
    `
  } catch {
    // ignore — telemetry is advisory only
  }
}
