import {
  DEFAULT_FORGE_TEAM,
  type ForgeAssignmentVariant,
  type ForgePositionAssignment,
  type ForgeTeam,
} from './team'

/**
 * Cheap plumbing/smoke roster.
 *
 * Every model-backed role uses DeepSeek Flash through the proven OpenCode
 * harness. Deterministic Assay/QA is deliberately unchanged and model-free.
 *
 * This is intentionally a team-map change only: lane behavior, routing,
 * permissions, worktree isolation and QA arithmetic do not change.
 */
export function buildForgeSmokeFlashTeam(
  team: ForgeTeam = DEFAULT_FORGE_TEAM,
): ForgeTeam {
  const cheap = (variant: ForgeAssignmentVariant): ForgeAssignmentVariant => ({
    ...variant,
    playerId: 'deepseek-flash',
    harnessId: 'opencode',
    lineage: 'deepseek-volume',
  })

  const mapAssignment = (
    assignment: ForgePositionAssignment,
  ): ForgePositionAssignment => {
    if (assignment.position === 'assay') return assignment
    return {
      ...cheap(assignment),
      position: assignment.position,
      ...(assignment.upgrade ? { upgrade: cheap(assignment.upgrade) } : {}),
      ...(assignment.emergency ? { emergency: cheap(assignment.emergency) } : {}),
    }
  }

  return {
    ...team,
    id: `${team.id}-smoke-flash`,
    name: `${team.name} Smoke Flash`,
    assignments: Object.fromEntries(
      Object.entries(team.assignments).map(([position, assignment]) => [
        position,
        mapAssignment(assignment),
      ]),
    ) as ForgeTeam['assignments'],
  }
}
