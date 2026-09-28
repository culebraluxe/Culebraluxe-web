// ---------------------------------------------------------------------------
// ⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
// The TypeScript engine and its libraries were deleted in the 2026-09 Rust port, so this
// file cannot load: the modules it imports from are gone. It is kept as REFERENCE ONLY,
// so the behaviour it describes can be translated into Rust when that behaviour is wanted.
// Reviving it in place is forbidden — see AGENTS.md ("legacy/ is out of scope") and
// docs/agent/BROKEN-TS-INVENTORY.md for the priority list and each capability's Rust home.
// ---------------------------------------------------------------------------
// ENG-FORGE-SPLIT-DOGFOOD-01 — seed the Story Board story that proves the SPLIT lane.
//
// Deliberately shaped as TWO independent, separately-provable units with disjoint
// files so a truthful Lead proposes SPLIT (and the fork/join/provenance/gate path
// actually executes), while staying small enough to dogfood safely in DEV.
import { createStoryboardStory, updateStoryboardStory } from '@/legacy/db/storyboard'

const id = 'ENG-FORGE-SPLIT-DOGFOOD-01'

const ASSAY_COMMANDS = [
  '- `pnpm exec tsx --test workflow_app/tests/forge-lead-routing-split.test.ts`',
  '- `pnpm exec tsx --test workflow_app/tests/forge-split-join.test.ts`',
].join('\n')

const ACCEPTANCE_CRITERIA = [
  'smithContractFromAssignment sets prohibitedScope to the sibling assignments surfaces, so the machine contract forbids editing a sibling branch (not only the rendered prose).',
  'splitJoinHoldReasons reports duplicated terminal child outcomes as a named reason instead of silently collapsing them.',
  'Both units are verifiable independently: each has its own focused test file and passing command.',
  'No other behavior changes: existing lead-routing and split-join tests keep passing.',
].join('\n')

async function main() {
  const fields = {
    id,
    workstream: 'ENGINEERING',
    title: 'Forge SPLIT lane: sibling-scope contract + duplicate join accounting',
    priority: 'Medium',
    status: 'Planned',
    notes: 'Packet: docs/agent/packets/ENG-FORGE-SPLIT-DOGFOOD-01.md (two independent units; dogfood for the SPLIT lane)',
    batch: null,
    goal:
      'Two independent hardening actions for the SPLIT lane: (1) the Smith execution contract must carry the SIBLING surfaces it may not edit, so isolation is machine-enforced rather than prose-only; (2) the pre-lead_post join gate must report duplicated terminal child outcomes instead of collapsing them silently. The two units touch different files and are independently provable.',
    scope:
      'Unit A: workflow_app/forge/forge-split-handoff.ts + workflow_app/tests/forge-lead-routing-split.test.ts. Unit B: workflow_app/forge/split-join.ts + workflow_app/tests/forge-split-join.test.ts.',
    dependencies: null,
    preconditions: null,
    architectBrief: null,
    contextRefs: 'docs/agent/packets/ENG-FORGE-SPLIT-DOGFOOD-01.md',
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
