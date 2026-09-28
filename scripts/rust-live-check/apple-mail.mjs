// Live check for the Apple Mail port (`rust/cli` — `apple-sync mail-intake` + `mail-promote`).
//
//   node scripts/rust-live-check/apple-mail.mjs
//
// Unit tests prove the ported rules; only a real database proves the write path. Mail's own store
// needs macOS Full Disk Access, which a test machine may not have, so this check drives the real
// commands against DEV with a STUB extractor (`CULEBRALUXE_MAIL_EXTRACTOR`) whose JSON is the
// shape the real bridge emits. The rest of the chain is the shipped code: landing into
// `l_applemail`, the envelope-index checkpoint, evidence, reconciliation, interaction, refresh.
//
// Safe to re-run and it leaves DEV as it found it:
//   * the source account is synthetic (`verify-live-check@culebraluxe.test`), so its landing rows,
//     evidence row and checkpoint can never collide with a real Mail sync's and are always deleted;
//   * the interactions are keyed on synthetic Message-IDs, so they are always deleted.
import { spawnSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { devDb, repoRoot } from './_env.mjs'

const SOURCE_ACCOUNT = 'verify-live-check@culebraluxe.test'
const SOURCE = 'icloud_mail'
// The landed replay identity is what the interaction is keyed on (`message-id:<rfc>`), not the
// bare Message-ID: one identity serves both the landing table and the canonical event.
const INBOUND_MESSAGE_ID = 'verify-live-check|apple-mail|inbound'
const OUTBOUND_MESSAGE_ID = 'verify-live-check|apple-mail|outbound'
const INBOUND_ID = `message-id:${INBOUND_MESSAGE_ID}`
const OUTBOUND_ID = `message-id:${OUTBOUND_MESSAGE_ID}`

const failures = []
function check(label, actual, expected) {
  const ok = JSON.stringify(actual) === JSON.stringify(expected)
  console.log(
    `${ok ? 'ok  ' : 'FAIL'} ${label}: ${JSON.stringify(actual)}${ok ? '' : ` (expected ${JSON.stringify(expected)})`}`,
  )
  if (!ok) failures.push(label)
}

function runCli(args, env) {
  const result = spawnSync(
    'cargo',
    ['run', '-q', '--manifest-path', resolve(repoRoot, 'rust/Cargo.toml'), '-p', 'cli', '--', ...args],
    { cwd: repoRoot, env: { ...process.env, ...env }, encoding: 'utf8' },
  )
  const stdout = result.stdout ?? ''
  const stderr = result.stderr ?? ''
  if (result.status !== 0) {
    console.error(`command failed: ${args.join(' ')}\n${stdout}\n${stderr}`)
  }
  return { status: result.status, stdout, stderr }
}

function lastJson(stdout) {
  const line = stdout
    .split('\n')
    .reverse()
    .find((row) => row.trim().startsWith('{'))
  return line ? JSON.parse(line) : null
}

/** A stub in the extractor's own protocol: one complete page, no cursor. */
function stubExtractor(dir, occurredAt, counterparty) {
  const path = join(dir, 'stub-extractor.py')
  const page = {
    ok: true,
    source: 'apple_mail_envelope_index',
    account: SOURCE_ACCOUNT,
    accountId: 'live-check-stub',
    mailVersion: 'V10',
    database: '/live-check/stub',
    band: '0-1',
    since: '2026-01-01T00:00:00Z',
    before: null,
    pageSize: 500,
    epoch: 'unix',
    mailboxes: { inbox: ['INBOX'], sent: ['Sent'] },
    records: [
      {
        mailbox: 'inbox',
        mailboxName: 'INBOX',
        localId: 900001,
        messageId: INBOUND_MESSAGE_ID,
        occurredAt,
        sender: `Live Check <${counterparty}>`,
        to: [{ address: 'lisa@culebraluxe.com', name: null }],
        cc: [],
        bcc: [],
        subject: 'live check inbound',
      },
      {
        mailbox: 'sent',
        mailboxName: 'Sent',
        localId: 900002,
        messageId: OUTBOUND_MESSAGE_ID,
        occurredAt,
        sender: 'lisa@culebraluxe.com',
        to: [{ address: counterparty, name: 'Live Check' }],
        cc: [],
        bcc: [],
        subject: 'live check outbound',
      },
    ],
    nextCursor: null,
    complete: true,
    verify: false,
  }
  writeFileSync(join(dir, 'page.json'), JSON.stringify(page))
  // The stub speaks the extractor's protocol by printing the page as JSON. Python is used
  // because that is what the real bridge is; the payload comes from a file so the stub stays
  // valid Python (JSON's true/false/null are not Python literals).
  writeFileSync(
    path,
    'import json, os\nprint(json.dumps(json.load(open(os.path.join(os.path.dirname(__file__), "page.json")))))\n',
  )
  return path
}

const db = await devDb()
let createdEvidence = 0

/** One unambiguous DEV email identity, so reconciliation has exactly one owner to find. */
async function devEmailIdentity() {
  const { rows } = await db.query(
    `select pi.person_id::text as person_id, lower(btrim(pi.identity_value)) as email
       from person_identity pi
       join person p on p.id = pi.person_id
      where p.archived_at is null and pi.identity_type = 'email'
        and pi.identity_value ~ '^[^@[:space:]]+@[^@[:space:]]+\\.[^@[:space:]]+$'
      group by pi.person_id, lower(btrim(pi.identity_value))
     having count(*) = 1
      order by lower(btrim(pi.identity_value)) asc
      limit 1`,
  )
  return rows[0]
}

const identity = await devEmailIdentity()
if (!identity) {
  console.error('SKIP: no unambiguous email identity in DEV to link a synthetic counterparty to.')
  await db.end()
  process.exit(0)
}
console.log(`linking a synthetic mail counterparty to DEV person ${identity.person_id} (${identity.email})`)

const dir = mkdtempSync(join(tmpdir(), 'apple-mail-live-check-'))
const occurredAt = new Date(Date.now() - 60 * 60 * 1000).toISOString()
const extractor = stubExtractor(dir, occurredAt, identity.email)
const env = {
  APP_ENV: 'development',
  CULEBRALUXE_MAIL_EXTRACTOR: extractor,
  MAIL_APP_ACCOUNTS: SOURCE_ACCOUNT,
  APPLE_MAIL_ACCOUNT_ID: 'live-check-stub',
  EMAIL_INTERNAL_ADDRESSES: 'lisa@culebraluxe.com',
}

try {
  const intake = runCli(['apple-sync', 'mail-intake', 'dev', '--band=0-1'], env)
  const tally = lastJson(intake.stdout)
  check('intake exit', intake.status, 0)
  check('intake landed 2 rows', tally?.summary?.[0]?.landed, 2)
  check('intake band complete', tally?.summary?.[0]?.complete, true)

  // The second run must be a pure replay: landing is replay-safe, so nothing lands twice.
  const replay = runCli(['apple-sync', 'mail-intake', 'dev', '--band=0-1'], env)
  const replayTally = lastJson(replay.stdout)
  check('replay exit', replay.status, 0)
  check('replay landed nothing', replayTally?.summary?.[0]?.landed, 0)
  check('replay replayed 2', replayTally?.summary?.[0]?.replayed, 2)

  const { rows: landedRows } = await db.query(
    `select count(*)::int as rows from l_applemail where source_account = $1`,
    [SOURCE_ACCOUNT],
  )
  check('l_applemail rows', landedRows[0].rows, 2)

  const { rows: checkpoint } = await db.query(
    `select records_landed::int as landed, records_replayed::int as replayed, status
       from integration_intake_checkpoint
      where source = 'applemail' and source_account = $1 and shard_key = 'band:0-1'`,
    [SOURCE_ACCOUNT],
  )
  check('checkpoint landed', checkpoint[0]?.landed, 2)
  check('checkpoint replayed', checkpoint[0]?.replayed, 2)
  check('checkpoint complete', checkpoint[0]?.status, 'complete')

  const promo = runCli(
    ['apple-sync', 'mail-promote', 'dev', '--days=3650', `--account=${SOURCE_ACCOUNT}`],
    env,
  )
  const promoTally = lastJson(promo.stdout)
  check('promote exit', promo.status, 0)
  check('promotion observations', promoTally?.observations, 2)
  check('promotion evidence rows', promoTally?.evidenceRows, 1)
  check('promotion interactions inserted', promoTally?.interactionsInserted, 2)
  check('promotion unlinked', promoTally?.unlinked, 0)

  const { rows: evidence } = await db.query(
    `select canonical_person_id::text as person_id, review_state, inbound_count::int as inbound,
            outbound_count::int as outbound
       from integration_relationship_evidence
      where source = $1 and source_account = $2 and source_identity_key = $3`,
    [SOURCE, SOURCE_ACCOUNT, identity.email],
  )
  createdEvidence = evidence.length
  check('evidence linked to the DEV person', evidence[0]?.person_id, identity.person_id)
  check('evidence review state', evidence[0]?.review_state, 'exact_linked')
  check('evidence counts inbound/outbound', [evidence[0]?.inbound, evidence[0]?.outbound], [1, 1])

  const { rows: interactions } = await db.query(
    `select source_external_id, channel, event_type, direction, title
       from interaction
      where source_system = $1 and source_external_id = any($2::text[])
      order by source_external_id asc`,
    [SOURCE, [INBOUND_ID, OUTBOUND_ID]],
  )
  check('interactions written', interactions.length, 2)
  check('interaction channel', interactions[0]?.channel, 'email')
  check('interaction event types', interactions.map((row) => row.event_type), [
    'email_received',
    'email_sent',
  ])
  check('interaction directions', interactions.map((row) => row.direction), ['inbound', 'outbound'])
  check('interaction title', interactions[0]?.title, 'live check inbound')

  // The promotion is replay-safe too: a second pass must insert nothing.
  const promoReplay = runCli(
    ['apple-sync', 'mail-promote', 'dev', '--days=3650', `--account=${SOURCE_ACCOUNT}`],
    env,
  )
  const promoReplayTally = lastJson(promoReplay.stdout)
  check('promotion replay inserted 0', promoReplayTally?.interactionsInserted, 0)
  check('promotion replay replayed 2', promoReplayTally?.interactionsReplayed, 2)
} finally {
  const cleanup = [
    db.query(
      `delete from interaction where source_system = $1 and source_external_id = any($2::text[])`,
      [SOURCE, [INBOUND_ID, OUTBOUND_ID]],
    ),
  ]
  if (createdEvidence > 0) {
    cleanup.push(
      db.query(
        `delete from integration_relationship_evidence where source = $1 and source_account = $2`,
        [SOURCE, SOURCE_ACCOUNT],
      ),
    )
  }
  cleanup.push(db.query(`delete from l_applemail where source_account = $1`, [SOURCE_ACCOUNT]))
  cleanup.push(
    db.query(
      `delete from integration_intake_checkpoint
        where source = 'applemail' and source_account = $1 and shard_key = 'band:0-1'`,
      [SOURCE_ACCOUNT],
    ),
  )
  await cleanup[0]
  for (const query of cleanup.slice(1)) {
    await query
  }
  rmSync(dir, { recursive: true, force: true })
  await db.end()
}

if (failures.length > 0) {
  console.error(`\n${failures.length} live check(s) failed: ${failures.join(', ')}`)
  process.exit(1)
}
console.log('\napple mail live check passed (DEV, cleaned up)')
