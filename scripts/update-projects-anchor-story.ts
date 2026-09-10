// ENG-PROJECTS-ANCHOR-01 — seed the Story Board story for the Forge dogfood run.
//
// Creates the story with the acceptance criteria + assay recipe the Ready gate
// requires (a QA-applicable story must not leave Planned without both), or updates
// it in place if it already exists.
//
//   node --env-file=.env.local --import tsx scripts/update-projects-anchor-story.ts
import { createStoryboardStory, updateStoryboardStory } from '../db/storyboard'

const id = 'ENG-PROJECTS-ANCHOR-01'

const ASSAY_COMMANDS = [
  '- `pnpm exec tsx --test testv2/projects-service-projection.test.ts`',
  '- `pnpm exec tsc --noEmit`',
].join('\n')

const ACCEPTANCE_CRITERIA = [
  'A project whose row has no propertyId but whose WBS items carry a property entity anchor attaches that property\'s documents.',
  'A project whose row has no personId but whose WBS items carry a person entity anchor matches that person\'s activity entries.',
  'A project with neither a row anchor nor an entity anchor projects empty documents and activity.',
  'A project row anchor still wins when present.',
  'Existing projection tests still pass unchanged.',
].join('\n')

async function main() {
  const fields = {
    id,
    workstream: 'ENGINEERING',
    title: 'Projects workspace: Documents/Activity fall back to WBS entity anchors',
    priority: 'Medium',
    status: 'Planned',
    notes: 'Packet: docs/agent/packets/ENG-PROJECTS-ANCHOR-01.md',
    batch: null,
    goal:
      'Projects Pane 2 Documents/Activity render empty because the projection matches on the project row anchors while the real anchors live on the WBS items.',
    scope: 'ui/projects/service-projection.ts + testv2/projects-service-projection.test.ts',
    dependencies: null,
    preconditions: null,
    architectBrief: null,
    contextRefs: 'docs/agent/packets/ENG-PROJECTS-ANCHOR-01.md',
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
    try {
      await updateStoryboardStory(id, fields)
      console.log('updated', id)
    } catch (updateErr) {
      const u = updateErr as { code?: string; message?: string }
      console.error(`update failed (${u.code ?? 'unknown'}): ${u.message ?? updateErr}`)
      throw updateErr
    }
  }
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(err)
    process.exit(1)
  })
