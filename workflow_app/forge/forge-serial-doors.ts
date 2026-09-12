// ---------------------------------------------------------------------------
// FORGE-SMITH-DOOR-01 — the two serial doors, defined once so they cannot drift.
//
// DOOR 1 (launch): a serial Smith lane cannot start without an accepted Lead
//   assignment. Smith does not choose its own scope; the Lead routes the work.
// DOOR 2 (scope): a candidate that leaves its accepted assignment is a MISS —
//   the self-heal reprompt names the paths, and exhaustion is a HOLD thrown by the
//   runner, never by an Alert.
//
// They were written in two separate commits (b484301, 57b64b7), and Grok's 91
// review flagged the drift risk precisely: b484301 said "no assignment → nothing to
// enforce" while 57b64b7 said "no assignment → HOLD at launch", and if any path can
// start Smith with an empty assignment AND skip the scope lock, the lock is
// decorative again. So both decisions live HERE, with the exact strings, and the
// test names both doors. The runner cannot grow a third policy without failing
// `workflow_app/tests/forge-smith-door.test.ts`.
//
// Pure: no DB, no git, no filesystem — which is why both doors are testable
// without a mocked engine.
// ---------------------------------------------------------------------------

/**
 * The lane-`smith` nodes that EXECUTE work orders, and therefore require an
 * accepted Lead assignment. Taken from the role mapping: `smith_split_work` is the
 * one lane-smith node excluded, because a split child is gated by its own
 * assignment contract before launch, and `lead_solo_implement` is a different lane
 * that legitimately carries no assignment.
 */
export const SERIAL_SMITH_NODES: readonly string[] = [
  'smith',
  'repair_smith',
  'fast_smith',
  'fast_repair_smith',
]

/** Exact launch-door sentence, shared by the runner and the test so they cannot drift. */
export const NO_ASSIGNMENT_REASON = 'Smith does not choose its own scope'

/** Exact prefix on every scope-miss reason. */
export const SCOPE_MISS_PREFIX = 'smith-scope:'

export type SerialLaunchDecision = {
  allowed: boolean
  /** Present only when the lane is refused; already operator-ready. */
  reason: string | null
}

/**
 * DOOR 1 — may this serial Smith lane launch?
 *
 * Refuses only the four executing Smith roles, and only for a missing accepted
 * assignment. Everything else is allowed by this door: a split child has its own
 * contract gate, `lead_solo_implement` is assignment-free by design, and every
 * non-Smith lane is not this door's business.
 */
export function serialLaunchDoor(input: {
  nodeId: string
  hasAcceptedAssignment: boolean
}): SerialLaunchDecision {
  if (!SERIAL_SMITH_NODES.includes(input.nodeId)) return { allowed: true, reason: null }
  if (input.hasAcceptedAssignment) return { allowed: true, reason: null }
  return {
    allowed: false,
    reason:
      `Forge ${input.nodeId} HOLD: no accepted Lead assignment for the serial Smith lane. ` +
      `${NO_ASSIGNMENT_REASON} — the Lead routes the work orders (LEAD_ROUTING) before this lane may run.`,
  }
}

/**
 * DOOR 2 — what did this candidate do outside its accepted assignment?
 *
 * Returns the miss reasons that ride the existing bounded self-heal. An empty
 * array means nothing to report: either the candidate stayed in its lane, or there
 * was no declared scope to violate (which door 1 now prevents for this lane).
 */
export function serialScopeMissReasons(violations: readonly string[], owner: string): string[] {
  return violations.map((path) => `${SCOPE_MISS_PREFIX}${path} is outside ${owner}`)
}
