// READ-ONLY PROBE: what is actually on the board in PROD, so a QA plan is written against real rows
// rather than assumptions. Writes nothing.
import { listActiveWork, listStoryboardStories } from '@/legacy/db/storyboard'

async function main() {
  const [bench, stories] = await Promise.all([listActiveWork(), listStoryboardStories()])
  const list = stories ?? []

  console.log(`TOTAL STORIES: ${list.length}`)
  const byStatus = new Map<string, number>()
  for (const s of list) byStatus.set(s.status, (byStatus.get(s.status) ?? 0) + 1)
  for (const [status, n] of [...byStatus.entries()].sort((a, b) => b[1] - a[1])) {
    console.log(`  ${status.padEnd(14)} ${n}`)
  }

  console.log(`\nON THE BENCH (storyboard_active_work): ${bench.length}`)
  for (const s of bench) {
    console.log(`  ${s.id.padEnd(26)} ${String(s.status).padEnd(12)} ${s.title?.slice(0, 52) ?? ''}`)
  }

  // Age: the board's `updatedAt` is the best available "is this stale" signal from the story row.
  const cutoff = Date.now() - 1000 * 60 * 60 * 24 * 30
  const stale = list.filter((s) => s.updatedAt && Date.parse(s.updatedAt) < cutoff)
  console.log(`\nNOT TOUCHED IN 30+ DAYS: ${stale.length}`)
  const staleBench = bench.filter((s) => s.updatedAt && Date.parse(s.updatedAt) < cutoff)
  console.log(`  of those, ON THE BENCH: ${staleBench.length}  ${staleBench.map((s) => s.id).join(', ')}`)

  // A smoke candidate: newest open-ish story that is NOT on the bench and is not engine-owned.
  const candidates = list
    .filter((s) => s.status === 'In Progress' && !bench.some((b) => b.id === s.id))
    .sort((a, b) => (b.updatedAt ?? '').localeCompare(a.updatedAt ?? ''))
  console.log(`\nOPEN (In Progress, not on bench): ${candidates.length}`)
  for (const s of candidates.slice(0, 6)) {
    console.log(`  ${s.id.padEnd(26)} ${s.updatedAt?.slice(0, 10)} ${s.title?.slice(0, 50) ?? ''}`)
  }
}

void main()
