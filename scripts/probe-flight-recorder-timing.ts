// Live timing probe for the Flight Recorder read model (the cockpit's 504).
//
// Picks the deal with the MOST engine instances — the worst case the console can be asked for —
// and times getFlightRecorderTransaction(instanceId) the way the cockpit route calls it.
import { engineConfigured, engineSql } from '../workflow_app/engine-client'
import { getFlightRecorderTransaction } from '../workflow_app/flight-recorder-read'

async function main(): Promise<void> {
  if (!engineConfigured()) {
    console.log('engine not configured; nothing to measure')
    return
  }
  const esql = engineSql()

  const worst = await esql`
    select pi.subject_id as deal_id, count(*)::int as n
    from process_instances pi
    where pi.subject_type = 'deal'
    group by pi.subject_id
    order by n desc
    limit 1
  `
  const row = worst[0] as { deal_id?: string; n?: number } | undefined
  console.log(`worst-case deal: ${row?.deal_id ?? '(none)'} with ${row?.n ?? 0} instances`)

  const newest = await esql`
    select pi.id
    from process_instances pi
    where pi.subject_type = 'deal' and pi.subject_id = ${row?.deal_id ?? ''}
    order by pi.created_at desc
    limit 1
  `
  const instanceId = String((newest[0] as { id?: unknown } | undefined)?.id ?? '')
  console.log(`opening instance: ${instanceId}`)

  const started = Date.now()
  const tx = await getFlightRecorderTransaction(instanceId)
  const elapsed = Date.now() - started

  console.log(
    `elapsed=${elapsed}ms workflows=${tx?.workflows?.length ?? 0} events=${tx?.events?.length ?? 0}`,
  )
  console.log(elapsed < 10_000 ? 'VERDICT: within the platform function timeout' : 'VERDICT: STILL TOO SLOW')
}

main().catch((err) => {
  console.error('probe failed:', err)
  process.exitCode = 1
})
