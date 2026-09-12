// ---------------------------------------------------------------------------
// LANDING writes — the intake stop for every channel.
//
// MODEL (captain, 2026-09-11): landing -> promote -> warehouse.
//
//   Apple iMessage -> l_imessage     Google email -> l_email
//   Apple calls /
//   FaceTime       -> l_call         WhatsApp     -> l_whatsapp
//   Calendar       -> l_calendar
//
// The warehouse is `interaction` (+ relationship evidence), written by
// createInteraction. The loaders used to go STRAIGHT there, which is why the
// landing tables had no writers. These functions are that missing step.
//
// CONTRACT, identical for every channel so ONE promote path serves all of them:
//   * source_account + source_message_id is the replay key (every landing table
//     has a unique index on exactly that), so re-running lands nothing new;
//   * `raw` carries the untouched source payload — a landing row is evidence, not
//     interpretation;
//   * `ingested_at` is ours, never the source's clock.
//
// Repository boundary: values are normalized HERE (dates to timestamptz, objects
// to jsonb) so nothing above needs the source's transport shape. Nothing here
// makes a judgment call, merges, or resolves identity — that is promote's job.
//
// Every function returns TRUE when it INSERTED the row and FALSE when the record
// was already landed, so a caller can report inserted vs replayed honestly.
// ---------------------------------------------------------------------------

import type { QueryExecutor } from './query-executor'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

export type LandedImessage = {
  sourceAccount: string | null
  sourceMessageId: string
  conversationId?: string | null
  handle?: string | null
  /** 'outgoing' when the owner sent it. */
  direction?: string | null
  service?: string | null
  sentAt?: string | null
  text?: string | null
  raw: unknown
}

export async function landImessage(
  input: LandedImessage,
  execute?: QueryExecutor,
): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = (await q`
    insert into l_imessage (
      source_account, source_message_id, conversation_id, handle,
      direction, service, sent_at, text_content, raw
    ) values (
      ${input.sourceAccount}, ${input.sourceMessageId}, ${input.conversationId ?? null},
      ${input.handle ?? null}, ${input.direction ?? null}, ${input.service ?? null},
      ${input.sentAt ?? null}::timestamptz, ${input.text ?? null},
      ${JSON.stringify(input.raw)}::jsonb
    )
    on conflict (coalesce(source_account, ''), source_message_id) do nothing
    returning id
  `) as unknown as unknown[]
  return Array.isArray(rows) && rows.length > 0
}

/**
 * Set-based landing for a bounded batch of records.
 *
 * The single-row form pays ONE ROUND TRIP PER MESSAGE. Measured on this Mac over
 * the pooled Neon driver: 68.7ms per round trip, which is what the iMessage
 * materializer's 5 hours is actually made of (93,000 messages x 2 writes =
 * 186,000 trips), NOT database work - one set-based statement over 20,000 rows
 * takes 71ms in total. Batching at 500 per statement turns 93,000 trips into 186.
 *
 * Replay-safe exactly like the single-row form: `on conflict do nothing`, so
 * `returning` yields only the rows that were genuinely new.
 */
export async function landImessageBatch(
  inputs: readonly LandedImessage[],
  execute?: QueryExecutor,
): Promise<number> {
  if (inputs.length === 0) return 0
  const q = execute ?? (await executor())
  const rows = (await q`
    insert into l_imessage (
      source_account, source_message_id, conversation_id, handle,
      direction, service, sent_at, text_content, raw
    )
    select
      t.source_account, t.source_message_id, t.conversation_id, t.handle,
      t.direction, t.service, t.sent_at, t.text_content, (t.raw)::jsonb
    from unnest(
      ${inputs.map((input) => input.sourceAccount)}::text[],
      ${inputs.map((input) => input.sourceMessageId)}::text[],
      ${inputs.map((input) => input.conversationId ?? null)}::text[],
      ${inputs.map((input) => input.handle ?? null)}::text[],
      ${inputs.map((input) => input.direction ?? null)}::text[],
      ${inputs.map((input) => input.service ?? null)}::text[],
      ${inputs.map((input) => input.sentAt ?? null)}::timestamptz[],
      ${inputs.map((input) => input.text ?? null)}::text[],
      ${inputs.map((input) => JSON.stringify(input.raw ?? null))}::text[]
    ) as t(
      source_account, source_message_id, conversation_id, handle,
      direction, service, sent_at, text_content, raw
    )
    on conflict (coalesce(source_account, ''), source_message_id) do nothing
    returning id
  `) as unknown as unknown[]
  return Array.isArray(rows) ? rows.length : 0
}

export type LandedEmail = {
  sourceAccount: string | null
  sourceMessageId: string
  threadId?: string | null
  fromAddress?: string | null
  toAddress?: string | null
  subject?: string | null
  sentAt?: string | null
  bodyPreview?: string | null
  labels?: string[] | null
  raw: unknown
}

export async function landEmail(input: LandedEmail, execute?: QueryExecutor): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = (await q`
    insert into l_email (
      source_account, source_message_id, thread_id, from_address, to_address,
      subject, sent_at, body_preview, labels, raw
    ) values (
      ${input.sourceAccount}, ${input.sourceMessageId}, ${input.threadId ?? null},
      ${input.fromAddress ?? null}, ${input.toAddress ?? null}, ${input.subject ?? null},
      ${input.sentAt ?? null}::timestamptz, ${input.bodyPreview ?? null},
      ${input.labels ?? null}, ${JSON.stringify(input.raw)}::jsonb
    )
    on conflict (coalesce(source_account, ''), source_message_id) do nothing
    returning id
  `) as unknown as unknown[]
  return Array.isArray(rows) && rows.length > 0
}

export type LandedAppleMail = {
  sourceAccount: string | null
  /** 'message-id:<Message-ID>' or 'mail-local:<mailbox-kind>:<local-id>'. */
  sourceMessageId: string
  /** 'inbox' | 'sent' */
  mailboxKind?: string | null
  mailboxName?: string | null
  localId?: number | null
  messageId?: string | null
  occurredAt?: string | null
  sender?: string | null
  toRecipients?: unknown
  ccRecipients?: unknown
  bccRecipients?: unknown
  subject?: string | null
  /** The UNMODIFIED exporter record — never a normalized or derived object. */
  raw: unknown
}

/**
 * The Apple Mail replay identity. Deliberately the same rule the intake already
 * uses: prefer the RFC Message-ID, fall back to the Mail.app local identity.
 * Never random — a random id would make every re-run land a duplicate.
 */
export function appleMailReplayId(input: {
  messageId?: string | null
  mailboxKind?: string | null
  localId?: number | null
}): string | null {
  const messageId = (input.messageId ?? '').trim()
  if (messageId) return `message-id:${messageId}`
  if (input.mailboxKind && typeof input.localId === 'number') {
    return `mail-local:${input.mailboxKind}:${input.localId}`
  }
  return null
}

export async function landAppleMail(
  input: LandedAppleMail,
  execute?: QueryExecutor,
): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = (await q`
    insert into l_applemail (
      source_account, source_message_id, mailbox_kind, mailbox_name, local_id,
      message_id, occurred_at, sender, to_recipients, cc_recipients,
      bcc_recipients, subject, raw
    ) values (
      ${input.sourceAccount}, ${input.sourceMessageId}, ${input.mailboxKind ?? null},
      ${input.mailboxName ?? null}, ${input.localId ?? null}, ${input.messageId ?? null},
      ${input.occurredAt ?? null}::timestamptz, ${input.sender ?? null},
      ${input.toRecipients == null ? null : JSON.stringify(input.toRecipients)}::jsonb,
      ${input.ccRecipients == null ? null : JSON.stringify(input.ccRecipients)}::jsonb,
      ${input.bccRecipients == null ? null : JSON.stringify(input.bccRecipients)}::jsonb,
      ${input.subject ?? null}, ${JSON.stringify(input.raw)}::jsonb
    )
    on conflict (coalesce(source_account, ''), source_message_id) do nothing
    returning id
  `) as unknown as unknown[]
  return Array.isArray(rows) && rows.length > 0
}

export type LandedCall = {
  sourceAccount: string | null
  sourceMessageId: string
  handle?: string | null
  /** 'incoming' | 'outgoing' */
  direction?: string | null
  /** 'audio' | 'video' — video is FaceTime. */
  callType?: string | null
  answered?: boolean | null
  durationSeconds?: number | null
  startedAt?: string | null
  raw: unknown
}

export async function landCall(input: LandedCall, execute?: QueryExecutor): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = (await q`
    insert into l_call (
      source_account, source_message_id, handle, direction, call_type,
      answered, duration_seconds, started_at, raw
    ) values (
      ${input.sourceAccount}, ${input.sourceMessageId}, ${input.handle ?? null},
      ${input.direction ?? null}, ${input.callType ?? null}, ${input.answered ?? null},
      ${input.durationSeconds ?? null}, ${input.startedAt ?? null}::timestamptz,
      ${JSON.stringify(input.raw)}::jsonb
    )
    on conflict (coalesce(source_account, ''), source_message_id) do nothing
    returning id
  `) as unknown as unknown[]
  return Array.isArray(rows) && rows.length > 0
}

export type LandedWhatsapp = {
  sourceAccount: string | null
  sourceMessageId: string
  conversationId?: string | null
  fromAddress?: string | null
  toAddress?: string | null
  direction?: string | null
  messageType?: string | null
  text?: string | null
  mediaId?: string | null
  sentAt?: string | null
  raw: unknown
}

export async function landWhatsapp(
  input: LandedWhatsapp,
  execute?: QueryExecutor,
): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = (await q`
    insert into l_whatsapp (
      source_account, source_message_id, conversation_id, from_address, to_address,
      direction, message_type, text_content, media_id, sent_at, raw
    ) values (
      ${input.sourceAccount}, ${input.sourceMessageId}, ${input.conversationId ?? null},
      ${input.fromAddress ?? null}, ${input.toAddress ?? null}, ${input.direction ?? null},
      ${input.messageType ?? null}, ${input.text ?? null}, ${input.mediaId ?? null},
      ${input.sentAt ?? null}::timestamptz, ${JSON.stringify(input.raw)}::jsonb
    )
    on conflict (coalesce(source_account, ''), source_message_id) do nothing
    returning id
  `) as unknown as unknown[]
  return Array.isArray(rows) && rows.length > 0
}

export type LandedCalendarEvent = {
  sourceAccount: string | null
  sourceMessageId: string
  title?: string | null
  startsAt?: string | null
  endsAt?: string | null
  allDay?: boolean | null
  location?: string | null
  organizer?: string | null
  attendees?: unknown
  raw: unknown
}

export async function landCalendarEvent(
  input: LandedCalendarEvent,
  execute?: QueryExecutor,
): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = (await q`
    insert into l_calendar (
      source_account, source_message_id, title, starts_at, ends_at,
      all_day, location, organizer, attendees, raw
    ) values (
      ${input.sourceAccount}, ${input.sourceMessageId}, ${input.title ?? null},
      ${input.startsAt ?? null}::timestamptz, ${input.endsAt ?? null}::timestamptz,
      ${input.allDay ?? null}, ${input.location ?? null}, ${input.organizer ?? null},
      ${input.attendees == null ? null : JSON.stringify(input.attendees)}::jsonb,
      ${JSON.stringify(input.raw)}::jsonb
    )
    on conflict (coalesce(source_account, ''), source_message_id) do nothing
    returning id
  `) as unknown as unknown[]
  return Array.isArray(rows) && rows.length > 0
}
