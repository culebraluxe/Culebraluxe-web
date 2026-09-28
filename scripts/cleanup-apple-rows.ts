// ---------------------------------------------------------------------------
// ⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
// The TypeScript engine and its libraries were deleted in the 2026-09 Rust port, so this
// file cannot load: the modules it imports from are gone. It is kept as REFERENCE ONLY,
// so the behaviour it describes can be translated into Rust when that behaviour is wanted.
// Reviving it in place is forbidden — see AGENTS.md ("legacy/ is out of scope") and
// docs/agent/BROKEN-TS-INVENTORY.md for the priority list and each capability's Rust home.
// ---------------------------------------------------------------------------
import { createPoolExecutor } from './lib/pool-executor'
const url = process.env.DATABASE_URL_DEV ?? process.env.DATABASE_URL ?? ''
async function main() {
  const { execute, end } = createPoolExecutor(url)
  const p = (await execute`select id from person where display_name = 'REL-INTEL Real Proof Person'`) as { id: string }[]
  console.log('orphan proof persons:', p.length)
  for (const r of p) {
    await execute`delete from person_identity where person_id = ${r.id}`
    await execute`delete from person where id = ${r.id}`
  }
  const r = (await execute`select count(*)::int as n from integration_relationship_evidence where source = 'apple_messages'`) as { n: number }[]
  console.log('leftover apple_messages rows:', r[0].n)
  await execute`delete from integration_relationship_evidence where source = 'apple_messages'`
  const r2 = (await execute`select count(*)::int as n from integration_relationship_evidence where source = 'apple_messages'`) as { n: number }[]
  console.log('after cleanup:', r2[0].n)
  await end()
}
main().then(() => process.exit(0)).catch((e) => { console.error(e); process.exit(1) })
