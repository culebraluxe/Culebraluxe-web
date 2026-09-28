// ---------------------------------------------------------------------------
// ⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
// The TypeScript engine and its libraries were deleted in the 2026-09 Rust port, so this
// file cannot load: the modules it imports from are gone. It is kept as REFERENCE ONLY,
// so the behaviour it describes can be translated into Rust when that behaviour is wanted.
// Reviving it in place is forbidden — see AGENTS.md ("legacy/ is out of scope") and
// docs/agent/BROKEN-TS-INVENTORY.md for the priority list and each capability's Rust home.
// ---------------------------------------------------------------------------
// READ-ONLY PROBE: what the engine has waiting / running RIGHT NOW, so a smoke test's expected
// before-and-after is known before anyone clicks. Writes nothing.
import { listActiveAgentWorkItems, listStaleAgentWork } from '@/legacy/db/agent-work'

async function main() {
  const open = await listActiveAgentWorkItems()
  console.log(`OPEN WORK ITEMS (agent_work_item, not finished): ${open.length}`)
  for (const w of open.slice(0, 12)) {
    console.log(
      `  ${String(w.storyId).padEnd(30)} ${String(w.state).padEnd(10)} ${String(w.updatedAt ?? '').slice(0, 16)}`,
    )
  }
  const stale = await listStaleAgentWork(15)
  console.log(`STALE (no touch in 15+ minutes): ${stale.length}`)
  for (const w of stale.slice(0, 8)) {
    console.log(`  ${String(w.storyId).padEnd(30)} ${String(w.state)}`)
  }
}

void main()
