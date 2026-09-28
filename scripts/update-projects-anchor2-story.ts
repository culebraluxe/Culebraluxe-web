// ---------------------------------------------------------------------------
// ⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
// The TypeScript engine and its libraries were deleted in the 2026-09 Rust port, so this
// file cannot load: the modules it imports from are gone. It is kept as REFERENCE ONLY,
// so the behaviour it describes can be translated into Rust when that behaviour is wanted.
// Reviving it in place is forbidden — see AGENTS.md ("legacy/ is out of scope") and
// docs/agent/BROKEN-TS-INVENTORY.md for the priority list and each capability's Rust home.
// ---------------------------------------------------------------------------
// ENG-PROJECTS-ANCHOR-02 — seed the Story Board story for the LEAD-routing validation run.
import { createStoryboardStory, updateStoryboardStory } from '@/legacy/db/storyboard'

const id = 'ENG-PROJECTS-ANCHOR-02'

const ASSAY_COMMANDS = [
  '- `pnpm exec tsx --test testv2/projects-service-projection.test.ts`',
  '- `pnpm exec tsc --noEmit`',
].join('\n')

const ACCEPTANCE_CRITERIA = [
  'A project whose row carries a person/property anchor reports anchorSource row.',
  'A project whose row has no anchor but whose WBS items carry an entity anchor reports anchorSource wbs.',
  'A project with neither reports anchorSource none.',
  'Existing projection tests still pass unchanged.',
].join('\n')

async function main() {
  const fields = {
    id,
    workstream: 'ENGINEERING',
    title: 'Projects projection: report the effective anchor source',
    priority: 'Medium',
    status: 'Planned',
    notes: 'Packet: docs/agent/packets/ENG-PROJECTS-ANCHOR-02.md',
    batch: null,
    goal:
      'ProjectPlan exposes where its documents/activity anchors came from (project row, WBS entity anchors, or none) so an empty pane can be explained rather than looking broken.',
    scope: 'ui/projects/model.ts + ui/projects/service-projection.ts + testv2/projects-service-projection.test.ts',
    dependencies: null,
    preconditions: null,
    architectBrief: null,
    contextRefs: 'docs/agent/packets/ENG-PROJECTS-ANCHOR-02.md',
    acceptanceCriteria: ACCEPTANCE_CRITERIA,
    postconditions: null,
    operatingSurface: 'TECH',
    completion: 0,
    rollup: false,
    plannedStartAt: null,
    actualStartAt: null,
    completedAt: null,
    testMode: 'SCOPED',
    assayCommands: ASSAY_COMMANDS,
  }

  try {
    await createStoryboardStory(fields)
    console.log('created', id)
  } catch (err) {
    const e = err as { code?: string; message?: string }
    console.error(`create failed (${e.code ?? 'unknown'}): ${e.message ?? err}`)
    await updateStoryboardStory(id, fields)
    console.log('updated', id)
  }
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(err)
    process.exit(1)
  })
