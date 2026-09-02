// Metadata-only Gmail latest-context sync.
//
// For every exact-linked Gmail relationship identity, fetch a bounded window
// of newest Gmail metadata and materialize the newest unambiguous direct email
// carrying a subject. No body, snippet, attachment, or raw MIME is requested.
import { getRelationshipEvidenceRows } from '../db/relationship-evidence'
import { createInteraction } from '../db/interactions'
import type { QueryExecutor } from '../db/query-executor'
import {
  gmailMetadataToContext,
  type GmailMetadataMessage,
} from '../lib/relationship-intel/gmail-latest-context'
import { createPoolExecutor } from './lib/pool-executor'

type EnvTarget = 'dev' | 'prod'

function requiredEnv(key: string): string {
  const value = process.env[key]?.trim()
  if (!value) throw new Error(`Missing required environment variable: ${key}`)
  return value
}

function targetDatabaseUrl(target: EnvTarget): string {
  return requiredEnv(target === 'prod' ? 'DATABASE_URL_PROD' : 'DATABASE_URL_DEV')
}

function parseTarget(raw: string | undefined): EnvTarget {
  if (raw === 'dev' || raw === 'prod') return raw
  throw new Error('Usage: gmail-metadata-sync.ts <dev|prod>')
}

async function accessToken(): Promise<string> {
  const response = await fetch('https://oauth2.googleapis.com/token', {
    method: 'POST',
    headers: { 'content-type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({
      client_id: requiredEnv('GOOGLE_CLIENT_ID'),
      client_secret: requiredEnv('GOOGLE_CLIENT_SECRET'),
      refresh_token: requiredEnv('GOOGLE_REFRESH_TOKEN'),
      grant_type: 'refresh_token',
    }),
  })
  const json = (await response.json()) as { access_token?: string; error?: string }
  if (!response.ok || !json.access_token) {
    throw new Error(
      `Google token refresh failed${json.error ? ` (${json.error})` : ''}. The refresh token must include gmail.readonly scope.`,
    )
  }
  return json.access_token
}

async function googleGet<T>(token: string, path: string): Promise<T> {
  const response = await fetch(`https://gmail.googleapis.com/gmail/v1/users/me/${path}`, {
    headers: { authorization: `Bearer ${token}`, accept: 'application/json' },
  })
  if (!response.ok) {
    const detail = (await response.text()).slice(0, 300)
    throw new Error(`Gmail API ${response.status}: ${detail}`)
  }
  return (await response.json()) as T
}

async function gmailProfile(token: string): Promise<{ emailAddress: string }> {
  return googleGet(token, 'profile')
}

async function newestContext(
  token: string,
  email: string,
  internalEmail: string,
  personId: string,
) {
  const q = `{from:${email} to:${email} cc:${email} bcc:${email}} -in:trash -in:spam -label:drafts`
  const list = await googleGet<{ messages?: Array<{ id: string }> }>(
    token,
    `messages?maxResults=25&q=${encodeURIComponent(q)}`,
  )
  for (const candidate of list.messages ?? []) {
    const params = new URLSearchParams({ format: 'metadata' })
    for (const name of ['From', 'To', 'Cc', 'Bcc', 'Subject']) {
      params.append('metadataHeaders', name)
    }
    const message = await googleGet<GmailMetadataMessage>(
      token,
      `messages/${encodeURIComponent(candidate.id)}?${params.toString()}`,
    )
    const mapped = gmailMetadataToContext(message, email, internalEmail, personId)
    if (mapped.ok) return mapped.interaction
  }
  return null
}

export async function runGmailMetadataSync(
  target: EnvTarget,
  execute: QueryExecutor,
): Promise<void> {
  console.log(`=== Gmail metadata latest-context sync (${target.toUpperCase()}) ===`)
  console.log('privacy: headers only; no bodies, snippets, attachments, or raw MIME')

  const token = await accessToken()
  const profile = await gmailProfile(token)
  const internalEmail = profile.emailAddress.trim().toLowerCase()
  console.log(`source account: ${internalEmail}`)

  const evidence = (await getRelationshipEvidenceRows('gmail_contacts', execute))
    .filter((row) => row.reviewState === 'exact_linked' && row.canonicalPersonId)
  console.log(`exact-linked Gmail identities: ${evidence.length}`)

  let inserted = 0
  let replayed = 0
  let noContext = 0
  let errors = 0
  for (let index = 0; index < evidence.length; index += 1) {
    const row = evidence[index]
    try {
      const input = await newestContext(
        token,
        row.sourceIdentityKey,
        internalEmail,
        row.canonicalPersonId!,
      )
      if (!input) {
        noContext += 1
      } else {
        const result = await createInteraction(input, execute)
        if (result.created) inserted += 1
        else replayed += 1
      }
    } catch (error) {
      errors += 1
      console.error(`identity failed: ${row.sourceIdentityKey}: ${error instanceof Error ? error.message : String(error)}`)
    }
    const processed = index + 1
    if (processed % 10 === 0 || processed === evidence.length) {
      console.log(`progress: ${processed}/${evidence.length} identities | inserted=${inserted} replayed=${replayed} no_context=${noContext} errors=${errors}`)
    }
  }

  if (inserted > 0) {
    console.log('refreshing Client relationship read models')
    await execute`refresh materialized view concurrently mv_client_relationship_channels`
    await execute`refresh materialized view concurrently mv_client_directory`
    await execute`refresh materialized view concurrently mv_client_contact_history`
  }
  console.log(`complete: inserted=${inserted} replayed=${replayed} no_context=${noContext} errors=${errors}`)
  if (errors > 0) throw new Error(`Gmail metadata sync completed with ${errors} identity errors.`)
}

async function main() {
  const target = parseTarget(process.argv[2])
  const pool = createPoolExecutor(targetDatabaseUrl(target))
  try {
    await runGmailMetadataSync(target, pool.execute)
  } finally {
    await pool.end()
  }
}

if (process.argv[1]?.endsWith('gmail-metadata-sync.ts')) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error)
    process.exitCode = 1
  })
}
