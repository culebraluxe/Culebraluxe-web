// Live check for the Apple Messages intake port (`rust/cli` — `apple-sync messages-intake`).
//
//   node scripts/rust-live-check/apple-messages-intake.mjs
//
// Unit tests prove the ported rules; only a real database proves the write path. This runs the real
// command against DEV with a synthetic export package whose only tie to reality is one existing DEV
// person identity, so the exact-link -> landing -> interaction -> read-model chain actually executes.
//
// It is safe to re-run and it leaves DEV as it found it:
//   * the package's source account is synthetic (`verify-live-check@culebraluxe.test`), so the
//     evidence row it writes can never collide with a real Apple sync's row and is always deleted;
//   * the landing row is keyed by a synthetic message GUID, so it is always deleted;
//   * the interaction row is the real `latest:<person>:<channel>` key, which may already exist, so it
//     is deleted only when this run created it.
import { spawnSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { devDb, repoRoot } from './_env.mjs'

const SOURCE_ACCOUNT = 'verify-live-check@culebraluxe.test'
const MESSAGE_GUID = 'verify-live-check|apple-messages-intake|1'
const DATE_ISO = '2026-01-02T03:04:05.000Z'

const failures = []
function check(label, actual, expected) {
  const ok = JSON.stringify(actual) === JSON.stringify(expected)
  console.log(
    `${ok ? 'ok  ' : 'FAIL'} ${label}: ${JSON.stringify(actual)}${ok ? '' : ` (expected ${JSON.stringify(expected)})`}`,
  )
  if (!ok) failures.push(label)
}

function runIntake(dir) {
  const result = spawnSync(
    'cargo',
    [
      'run',
      '-q',
      '--manifest-path',
      resolve(repoRoot, 'rust/Cargo.toml'),
      '-p',
      'cli',
      '--',
      'apple-sync',
      'messages-intake',
      dir,
    ],
    { cwd: repoRoot, env: { ...process.env, APP_ENV: 'development' }, encoding: 'utf8' },
  )
  if (result.status !== 0) {
    throw new Error(`intake exited ${result.status}\n${result.stdout}\n${result.stderr}`)
  }
  const line = result.stdout
    .split('\n')
    .reverse()
    .find((row) => row.trim().startsWith('{'))
  return JSON.parse(line)
}

const db = await devDb()

/** One unambiguous DEV phone identity, so reconciliation has exactly one owner to find. */
async function devPhoneIdentity() {
  const { rows } = await db.query(
    `select pi.person_id::text as person_id, pi.identity_value
       from person_identity pi
       join person p on p.id = pi.person_id
      where p.archived_at is null and pi.identity_type = 'phone'
        and length(regexp_replace(pi.identity_value, '[^0-9]', '', 'g')) >= 10
      group by pi.person_id, pi.identity_value
     having count(*) = 1
      order by pi.identity_value asc
      limit 1`,
  )
  return rows[0]
}

const identity = await devPhoneIdentity()
if (!identity) {
  console.error('SKIP: no unambiguous phone identity in DEV to link a synthetic handle to.')
  await db.end()
  process.exit(0)
}
console.log(`linking a synthetic Apple handle to DEV person ${identity.person_id} (${identity.identity_value})`)

const dir = mkdtempSync(join(tmpdir(), 'apple-intake-live-check-'))
writeFileSync(
  join(dir, 'manifest.json'),
  JSON.stringify({
    exportVersion: 1,
    sourceSystem: 'apple_messages',
    minimumMessageDate: DATE_ISO,
    maximumMessageDate: DATE_ISO,
  }),
)
writeFileSync(
  join(dir, 'identities.jsonl'),
  `${JSON.stringify({
    rowid: 900001,
    id: identity.identity_value,
    country: 'US',
    service: 'iMessage',
    uncanonicalizedId: null,
    personCentricId: null,
  })}\n`,
)
writeFileSync(
  join(dir, 'messages.jsonl'),
  `${JSON.stringify({
    rowid: 900001,
    guid: MESSAGE_GUID,
    chatGuid: `any;-;${identity.identity_value}`,
    handleId: 900001,
    handleValue: identity.identity_value,
    service: 'iMessage',
    account: SOURCE_ACCOUNT,
    date: null,
    dateISO: DATE_ISO,
    isFromMe: 0,
    text: 'live check for the rust apple intake',
    hasAttachments: 0,
  })}\n`,
)

let createdInteraction = false
let createdEvidence = false
const interactionKey = `latest:${identity.person_id}:imessage`

try {
  const before = await db.query(
    `select
       (select count(*) from integration_relationship_evidence
         where source = 'apple_messages' and source_account = $1 and source_identity_key = $2) as evidence,
       (select count(*) from interaction
         where source_system = 'apple_messages' and source_external_id = $3) as interaction`,
    [SOURCE_ACCOUNT, identity.identity_value, interactionKey],
  )
  createdEvidence = Number(before.rows[0].evidence) === 0
  createdInteraction = Number(before.rows[0].interaction) === 0

  const first = runIntake(dir)
  check('first run target', first.target, 'dev')
  check('first run found an exact link', first.exactLinkedHandles >= 1, true)
  check('first run messages landed', first.landed, 1)
  check('first run group chats skipped', first.skippedGroupChat, 0)

  const evidence = await db.query(
    `select canonical_person_id::text as person_id, review_state, inbound_count, outbound_count
       from integration_relationship_evidence
      where source = 'apple_messages' and source_account = $1 and source_identity_key = $2`,
    [SOURCE_ACCOUNT, identity.identity_value],
  )
  check('evidence row written', evidence.rowCount, 1)
  check('evidence linked to the DEV person', evidence.rows[0]?.person_id, identity.person_id)
  check('evidence review state', evidence.rows[0]?.review_state, 'exact_linked')
  check(
    'evidence counted the message inbound',
    [evidence.rows[0]?.inbound_count, evidence.rows[0]?.outbound_count],
    [1, 0],
  )

  const landed = await db.query('select direction, service from l_imessage where source_message_id = $1', [
    MESSAGE_GUID,
  ])
  check('landing row written', landed.rowCount, 1)
  check('landing direction', landed.rows[0]?.direction, 'incoming')

  const interaction = await db.query(
    `select person_id::text as person_id, channel, event_type, direction
       from interaction where source_system = 'apple_messages' and source_external_id = $1`,
    [interactionKey],
  )
  check('latest interaction written', interaction.rowCount, 1)
  check('latest interaction person', interaction.rows[0]?.person_id, identity.person_id)
  check(
    'latest interaction key parts',
    [interaction.rows[0]?.channel, interaction.rows[0]?.event_type, interaction.rows[0]?.direction],
    ['imessage', 'message', 'inbound'],
  )

  const second = runIntake(dir)
  check('replay lands nothing new', second.landed, 0)
  check(
    'replay reports the interaction as already current',
    second.sourceRowsCurrent + second.sourceRowsUpdated >= 1,
    true,
  )
} finally {
  if (createdInteraction) {
    await db.query(`delete from interaction where source_system = 'apple_messages' and source_external_id = $1`, [
      interactionKey,
    ])
  }
  await db.query('delete from l_imessage where source_message_id = $1', [MESSAGE_GUID])
  if (createdEvidence) {
    await db.query(
      `delete from integration_relationship_evidence
        where source = 'apple_messages' and source_account = $1 and source_identity_key = $2`,
      [SOURCE_ACCOUNT, identity.identity_value],
    )
  } else {
    console.log('note: the evidence row for this identity already existed; left in place')
  }
  rmSync(dir, { recursive: true, force: true })
  await db.end()
}

if (failures.length > 0) {
  console.error(`\n${failures.length} check(s) failed: ${failures.join(', ')}`)
  process.exit(1)
}
console.log('\nall Apple Messages intake live checks passed (DEV left as found)')
