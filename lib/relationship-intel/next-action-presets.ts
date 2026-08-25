// ---------------------------------------------------------------------------
// CORE-DAILY-04 — reusable next-action presets (PURE).
//
// Context-aware next-action presets used from Catch-Up, Client and Contract.
// Each preset provides a sensible default title + a default due offset so Lisa
// can establish the next business action with minimal typing. Persistence stays
// in the canonical follow-up/task seam (CORE-DAILY-01); this module is only the
// human-friendly wording/timing vocabulary.
// ---------------------------------------------------------------------------

export type NextActionPresetCode =
  | 'call_back'
  | 'send_information'
  | 'schedule_showing'
  | 'prepare_offer'
  | 'check_financing'
  | 'follow_up_lawyer'
  | 'check_appraisal'
  | 'check_inspection'
  | 'check_closing_readiness'
  | 'custom_reminder'

export type NextActionPreset = {
  code: NextActionPresetCode
  label: string
  title: string
  /** default due offset in milliseconds (0 = due today/now) */
  defaultDueOffsetMs: number
}

const DAY = 24 * 60 * 60 * 1000

export const NEXT_ACTION_PRESETS: Record<NextActionPresetCode, NextActionPreset> = {
  call_back: { code: 'call_back', label: 'Call back', title: 'Call back', defaultDueOffsetMs: DAY },
  send_information: { code: 'send_information', label: 'Send information', title: 'Send information', defaultDueOffsetMs: DAY },
  schedule_showing: { code: 'schedule_showing', label: 'Schedule showing', title: 'Schedule showing', defaultDueOffsetMs: DAY },
  prepare_offer: { code: 'prepare_offer', label: 'Prepare offer', title: 'Prepare offer', defaultDueOffsetMs: 2 * DAY },
  check_financing: { code: 'check_financing', label: 'Check financing', title: 'Check financing', defaultDueOffsetMs: 2 * DAY },
  follow_up_lawyer: { code: 'follow_up_lawyer', label: 'Follow up with lawyer', title: 'Follow up with lawyer', defaultDueOffsetMs: 2 * DAY },
  check_appraisal: { code: 'check_appraisal', label: 'Check appraisal', title: 'Check appraisal', defaultDueOffsetMs: 2 * DAY },
  check_inspection: { code: 'check_inspection', label: 'Check inspection', title: 'Check inspection', defaultDueOffsetMs: 2 * DAY },
  check_closing_readiness: { code: 'check_closing_readiness', label: 'Check closing readiness', title: 'Check closing readiness', defaultDueOffsetMs: 3 * DAY },
  custom_reminder: { code: 'custom_reminder', label: 'Custom reminder', title: 'Custom reminder', defaultDueOffsetMs: DAY },
}

export const NEXT_ACTION_PRESET_CODES = Object.keys(NEXT_ACTION_PRESETS) as NextActionPresetCode[]

/** Resolve a preset's default due timestamp (ISO) from a base instant. */
export function presetDefaultDue(code: NextActionPresetCode, baseMs: number = Date.now()): string {
  const preset = NEXT_ACTION_PRESETS[code]
  return new Date(baseMs + preset.defaultDueOffsetMs).toISOString()
}
