// ENG-PROJECTS-ANCHOR-02 — seed the Story Board story for the LEAD-routing validation run.
import { createStoryboardStory, updateStoryboardStory } from '../db/storyboard'

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
