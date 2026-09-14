// READ-ONLY: the Cockpit's state in one command. Writes NOTHING.
//
//   pnpm forge:batch:status
//
// Checked by the captain against the screen: the board and this output should agree, and where they do
// not, the disagreement is the bug. Prints the batch table (the job stream), the engine's queue, and the
// bench — the three things the Cockpit exists to make visible.
import { listActiveAgentWorkItems } from '@/db/agent-work'
import { getStagingBatch, listForgeBatches } from '@/db/forge-batch'
import { listActiveWork, listStoryboardStories } from '@/db/storyboard'

async function main() {
  const [staging, batches, queue, bench, stories] = await Promise.all([
    getStagingBatch(),
    listForgeBatches(8),
    listActiveAgentWorkItems(),
    listActiveWork(),
    listStoryboardStories(),
  ])

  console.log('=== BATCH TABLE (the job stream: "if it is in the table it goes") ===')
  if (!staging) {
    console.log('  staging batch: none — nothing is staged right now')
  } else {
    console.log(`  staging batch ${staging.id} · ${staging.storyCount} story(ies) · status ${staging.status}`)
  }
  if (batches.length === 0) {
    console.log('  no batches recorded yet')
  } else {
    for (const b of batches) {
      const when =
        b.status === 'Scheduled'
          ? `fires ${b.scheduledFor ?? '?'}`
          : b.status === 'Fired'
            ? `fired ${b.firedAt ?? '?'}`
            : b.status.toLowerCase()
      console.log(
        `  ${b.id.slice(0, 8)}  ${b.status.padEnd(9)} ${when.padEnd(28)} ` +
          `${b.queuedCount}/${b.storyCount} queued${b.skippedCount ? ` · ${b.skippedCount} skipped` : ''}` +
          `${b.label ? `  (${b.label})` : ''}`,
      )
    }
  }

  console.log('\n=== ENGINE QUEUE (real time: what is queued or running) ===')
  if (queue.length === 0) {
    console.log('  empty — the engine is idle')
  } else {
    for (const w of queue) {
      console.log(`  ${String(w.storyId).padEnd(30)} ${String(w.state).padEnd(10)} ${String(w.updatedAt ?? '').slice(0, 19)}`)
    }
  }

  console.log('\n=== THE BOARD ===')
  const list = stories ?? []
  const batched = list.filter((s) => s.status === 'Batched').map((s) => s.id)
  const ready = list.filter((s) => s.status === 'Ready').map((s) => s.id)
  console.log(`  stories: ${list.length} total · batched ${batched.length} · handed to the engine (Ready) ${ready.length}`)
  console.log(`  work bench: ${bench.length}${bench.length ? ` (${bench.map((s) => s.id).join(', ')})` : ''}`)
  if (batched.length) console.log(`  batched on the board: ${batched.join(', ')}`)
  if (ready.length) console.log(`  Ready on the board:   ${ready.join(', ')}`)

  const boardVsTable =
    batched.length === (staging?.storyCount ?? 0) ? 'agree' : 'DISAGREE — this is a bug, report it'
  console.log(`\n  board vs table: ${boardVsTable}`)
}

void main()
