import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createServer, type Server, type IncomingMessage, type ServerResponse } from 'node:http'
import { once } from 'node:events'

// ---------------------------------------------------------------------------
// DOC-04 — BoldSign Integration adapter tests.
//
// Exercises the BoldSignSignatureProvider (DOC-03 SignatureProvider impl)
// against a FAKE BoldSign server (real HTTP, in-process) so the typed client
// (auth header, retries, timeouts) and the webhook HMAC verification run for
// real. The canonical signature seam (receipts + transaction_document +
// signature_request) runs on the same in-memory FakeDb the DOC-03 seam test
// uses, extended with the provider tables (bold_sign_request +
// bold_sign_webhook_event) — proving provider ids live ONLY there.
//
// Scoped per the runtime test policy: this file only — no full regression, no
// persistence harness.
// ---------------------------------------------------------------------------

import { createCommandRegistry } from '../../lib/commands/register'
import { CommandDispatcherImpl } from '../../lib/commands/dispatcher'
import { PostgresCommandReceiptRepository } from '../../db/command-receipt-repository'
import { SignatureApplication } from '../../lib/signature/application'
import { BoldSignSignatureProvider } from '../../lib/signature/boldsign/adapter'
import { BoldSignClient, type BoldSignClientDeps } from '../../lib/signature/boldsign/client'
import { loadBoldSignConfig, type BoldSignConfig } from '../../lib/signature/boldsign/config'
import { classifyBoldSignError, isTransientHttpStatus } from '../../lib/signature/boldsign/errors'
import {
  parseBoldSignWebhookPayload,
  signBoldSignWebhook,
  verifyBoldSignWebhookSignature,
} from '../../lib/signature/boldsign/webhook'
import { mapBoldSignWebhookEvent } from '../../lib/signature/boldsign/events'
import {
  BOLD_SIGN_DOCUMENT_STATUSES,
  mapProviderStatus,
} from '../../lib/signature/status-mapping'
import {
  getActiveSignatureRequestForDocument,
  getSignatureRequest,
} from '../../db/signature-request'
import {
  getBoldSignRequestBySignatureRequestId,
  getBoldSignWebhookEventByProviderEventId,
} from '../../db/bold-sign-request'
import type { SignatureRequest, SignatureRecipient } from '../../lib/signature/contracts'
import type { QueryExecutor } from '../../db/query-executor'
import type { TxRunner } from '../../db/tx'

type Row = Record<string, any>
const FIXED_NOW = () => new Date('2026-08-22T00:00:00.000Z')
const NOW_SEC = Math.floor(FIXED_NOW().getTime() / 1000)

const ACTIVE = ['requested', 'sent', 'viewed', 'signed']
function isActive(status: string): boolean {
  return ACTIVE.includes(status)
}

// ---------------------------------------------------------------------------
// FakeBoldSignServer — an in-process BoldSign API + webhook signer.
// ---------------------------------------------------------------------------

type FakeServerRequest = {
  method: string
  path: string
  query: URLSearchParams
  raw: string
  headers: Record<string, string | string[] | undefined>
}

type FakeEnvelope = {
  documentId: string
  status: string
  fileIds: string[]
  roles: unknown
  title: string | null
  message: string | null
}

class FakeBoldSignServer {
  static async start(options: { secret?: string; apiKey?: string } = {}): Promise<FakeBoldSignServer> {
    const server = new FakeBoldSignServer(options)
    server.httpServer = createServer((req, res) => void server.handle(req, res))
    server.httpServer.listen(0, '127.0.0.1')
    await once(server.httpServer, 'listening')
    const address = server.httpServer.address() as { port: number }
    server.baseUrl = `http://127.0.0.1:${address.port}`
    return server
  }

  readonly secret: string
  readonly apiKey: string
  baseUrl = ''
  envelopes = new Map<string, FakeEnvelope>()
  requests: FakeServerRequest[] = []
  private envelopeSeq = 0
  private failNextRules: Array<{ method: string; path: string; status: number; times: number }> = []
  private hangRule: { method: string; path: string; times: number } | null = null
  private httpServer: Server | null = null

  private constructor(options: { secret?: string; apiKey?: string }) {
    this.secret = options.secret ?? 'webhook-secret'
    this.apiKey = options.apiKey ?? 'test-api-key'
  }

  async stop(): Promise<void> {
    const server = this.httpServer
    if (!server) return
    server.closeAllConnections?.()
    server.close()
    await once(server, 'close').catch(() => undefined)
  }

  // -- test-driving helpers ---------------------------------------------------

  failNext(method: string, path: string, status: number, times = 1): void {
    this.failNextRules.push({ method, path, status, times })
  }

  hangNext(method: string, path: string, times = 1): void {
    this.hangRule = { method, path, times }
  }

  setEnvelopeStatus(documentId: string, status: string): void {
    const envelope = this.envelopes.get(documentId)
    if (!envelope) throw new Error(`Fake BoldSign: unknown envelope ${documentId}`)
    envelope.status = status
  }

  requestCount(method: string, path: string): number {
    return this.requests.filter((r) => r.method === method && r.path === path).length
  }

  /** Sign a raw webhook body exactly like BoldSign (t=<ts>, s0=<hmac-hex>). */
  signWebhook(body: string, timestampSeconds: number = NOW_SEC, secret = this.secret): string {
    return signBoldSignWebhook(body, secret, timestampSeconds)
  }

  // -- HTTP handling ----------------------------------------------------------

  private async handle(req: IncomingMessage, res: ServerResponse): Promise<void> {
    const chunks: Buffer[] = []
    for await (const chunk of req) chunks.push(chunk as Buffer)
    const raw = Buffer.concat(chunks).toString('utf8')
    const url = new URL(req.url ?? '/', this.baseUrl)
    const headers: Record<string, string | string[] | undefined> = {}
    for (const [key, value] of Object.entries(req.headers)) headers[key] = value
    this.requests.push({ method: req.method ?? '', path: url.pathname, query: url.searchParams, raw, headers })

    const hang = this.hangRule
    if (hang && hang.method === req.method && hang.path === url.pathname) {
      hang.times -= 1
      if (hang.times <= 0) this.hangRule = null
      return // never respond: the client timeout aborts the fetch
    }

    const fail = this.failNextRules.find((rule) => rule.method === req.method && rule.path === url.pathname)
    if (fail) {
      fail.times -= 1
      if (fail.times <= 0) this.failNextRules = this.failNextRules.filter((r) => r !== fail)
      res.writeHead(fail.status, { 'content-type': 'application/json' })
      res.end(JSON.stringify({ message: `Fake BoldSign error ${fail.status}` }))
      return
    }

    if (headers['x-api-key'] !== this.apiKey) {
      res.writeHead(401, { 'content-type': 'application/json' })
      res.end(JSON.stringify({ message: 'Unauthorized' }))
      return
    }

    if (req.method === 'POST' && url.pathname === '/v1/template/send') {
      let body: any
      try {
        body = JSON.parse(raw)
      } catch {
        res.writeHead(422, { 'content-type': 'application/json' })
        res.end(JSON.stringify({ message: 'Malformed body' }))
        return
      }
      const documentId = `env-${++this.envelopeSeq}`
      this.envelopes.set(documentId, {
        documentId,
        status: 'InProgress',
        fileIds: [`file-${documentId}`],
        roles: body.roles ?? null,
        title: body.title ?? null,
        message: body.message ?? null,
      })
      res.writeHead(201, { 'content-type': 'application/json' })
      res.end(JSON.stringify({ documentId }))
      return
    }

    // Direct document send (multipart/form-data) — the canonical send path.
    if (req.method === 'POST' && url.pathname === '/v1/document/send') {
      const documentId = `env-${++this.envelopeSeq}`
      // Extract the signers form field (a JSON string) from the multipart body
      // for the envelope record.
      const signersMatch = raw.match(/name="signers"\s*\r\n\r\n([\s\S]*?)\r\n--/)
      let signers: unknown = null
      if (signersMatch) {
        try {
          signers = JSON.parse(signersMatch[1])
        } catch {
          signers = null
        }
      }
      this.envelopes.set(documentId, {
        documentId,
        status: 'InProgress',
        fileIds: [`file-${documentId}`],
        roles: signers,
        title: null,
        message: null,
      })
      res.writeHead(201, { 'content-type': 'application/json' })
      res.end(JSON.stringify({ documentId }))
      return
    }

    if (req.method === 'GET' && url.pathname === '/v1/document/properties') {
      const documentId = url.searchParams.get('documentId')
      const envelope = documentId ? this.envelopes.get(documentId) : undefined
      if (!envelope) {
        res.writeHead(404, { 'content-type': 'application/json' })
        res.end(JSON.stringify({ message: 'Document not found' }))
        return
      }
      res.writeHead(200, { 'content-type': 'application/json' })
      res.end(
        JSON.stringify({
          documentId: envelope.documentId,
          status: envelope.status,
          files: envelope.fileIds.map((id) => ({ id, documentName: id })),
        }),
      )
      return
    }

    if (req.method === 'POST' && url.pathname === '/v1/document/revoke') {
      let body: any
      try {
        body = JSON.parse(raw)
      } catch {
        res.writeHead(422, { 'content-type': 'application/json' })
        res.end(JSON.stringify({ message: 'Malformed body' }))
        return
      }
      const envelope = this.envelopes.get(body?.documentId)
      if (!envelope) {
        res.writeHead(404, { 'content-type': 'application/json' })
        res.end(JSON.stringify({ message: 'Document not found' }))
        return
      }
      envelope.status = 'Revoked'
      res.writeHead(200, { 'content-type': 'application/json' })
      res.end('{}')
      return
    }

    // DOC-05 — signed-artifact download: base64-encoded file per BoldSign's
    // "Download a Document as Base64" response shape.
    if (req.method === 'GET' && url.pathname === '/v1/document/download') {
      const documentId = url.searchParams.get('documentId')
      const envelope = documentId ? this.envelopes.get(documentId) : undefined
      if (!envelope) {
        res.writeHead(404, { 'content-type': 'application/json' })
        res.end(JSON.stringify({ message: 'Document not found' }))
        return
      }
      const bytes = Buffer.from(`signed:${envelope.documentId}:pdf`, 'utf8')
      res.writeHead(200, { 'content-type': 'application/json' })
      res.end(
        JSON.stringify({
          file: bytes.toString('base64'),
          fileName: `${envelope.documentId}-signed.pdf`,
          mimeType: 'application/pdf',
        }),
      )
      return
    }

    res.writeHead(404, { 'content-type': 'application/json' })
    res.end(JSON.stringify({ message: `Fake BoldSign: no route ${req.method} ${url.pathname}` }))
  }
}

// ---------------------------------------------------------------------------
// FakeDb — in-memory canonical seam + BoldSign provider tables.
// ---------------------------------------------------------------------------

class FakeDb {
  documents: Row[] = []
  requests: Row[] = []
  receipts: Row[] = []
  boldSign: Row[] = []
  webhookEvents: Row[] = []
  media: Row[] = []
  seq = 0
  now = '2026-08-22T00:00:00.000Z'

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  runner: TxRunner = async (cb) => cb(this.tx)

  tx: QueryExecutor = (strings, ...params) => {
    const t = this.norm(
      strings.reduce(
        (acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''),
        '',
      ),
    )
    const p = params as any[]

    // ---- workflow_command_receipt (claim-first) ----
    if (t.includes('insert into workflow_command_receipt') && t.includes('on conflict')) {
      if (this.receipts.some((r) => r.command_id === p[0])) return Promise.resolve([])
      this.receipts.push({ command_id: p[0], outcome: 'pending', aggregate_id: null, message: null, actor_app_user_id: p[1] ?? null })
      return Promise.resolve([{ command_id: p[0] }])
    }
    if (t.includes('update workflow_command_receipt set outcome =')) {
      const r = this.receipts.find((x) => x.command_id === p[4])
      if (r) {
        r.outcome = p[0]
        r.aggregate_id = p[1]
        r.message = p[2]
        r.actor_app_user_id = p[3] ?? null
      }
      return Promise.resolve([])
    }
    if (
      t.includes('from workflow_command_receipt') &&
      t.includes('where command_id')
    ) {
      const r = this.receipts.find((x) => x.command_id === p[0])
      return Promise.resolve(
        r
          ? [{ command_id: r.command_id, outcome: r.outcome, aggregate_id: r.aggregate_id, message: r.message, actor_app_user_id: r.actor_app_user_id ?? null }]
          : [],
      )
    }

    // ---- transaction_document (existence lookup only) ----
    if (t.includes('select id from transaction_document') && t.includes('where id =')) {
      const doc = this.documents.find((d) => d.id === p[0])
      return Promise.resolve(doc ? [{ id: doc.id }] : [])
    }

    // ---- direct-PDF send: transaction_document -> media (unsigned PDF bytes) ----
    if (t.includes('from transaction_document d') && t.includes('join media m')) {
      const doc = this.documents.find((d) => d.id === p[0])
      const mediaRow = doc?.media_id
        ? this.media.find((m) => m.id === doc.media_id)
        : undefined
      return Promise.resolve(
        mediaRow
          ? [{ file_data: mediaRow.file_data, filename: mediaRow.filename, mime_type: mediaRow.mime_type }]
          : [],
      )
    }

    // ---- signature_request (canonical, provider-free) ----
    if (t.includes('insert into signature_request')) {
      const dup = this.requests.find((r) => r.transaction_document_id === p[0] && isActive(r.status))
      if (dup) return Promise.resolve([]) // on conflict ... do nothing
      this.seq += 1
      const row = {
        id: `sig-${this.seq}`,
        transaction_document_id: p[0],
        status: 'requested',
        message: p[1],
        created_by_user_id: p[2],
        created_at: this.now,
        updated_at: this.now,
      }
      this.requests.push(row)
      return Promise.resolve([row])
    }
    if (t.includes('update signature_request set status =')) {
      const r = this.requests.find((x) => x.id === p[1] && x.status === p[2])
      if (!r) return Promise.resolve([])
      r.status = p[0]
      r.updated_at = this.now
      return Promise.resolve([{ ...r }])
    }
    if (t.includes('from signature_request')) {
      if (t.includes('where id =')) return Promise.resolve(this.requests.filter((r) => r.id === p[0]))
      if (t.includes('where transaction_document_id =')) {
        return Promise.resolve(
          this.requests
            .filter((r) => r.transaction_document_id === p[0])
            .sort((a, b) => (a.created_at < b.created_at ? -1 : 1)),
        )
      }
    }

    // ---- bold_sign_request (provider table — DOC-04) ----
    // successful-send insert (on conflict envelope_id where not null do nothing)
    if (t.includes('insert into bold_sign_request') && t.includes('on conflict (envelope_id)')) {
      const dup = this.boldSign.find((r) => r.envelope_id === p[1])
      if (dup) return Promise.resolve([])
      const row = {
        signature_request_id: p[0],
        envelope_id: p[1],
        document_ids: Array.isArray(p[2]) ? [...p[2]] : [],
        status: p[3],
        last_error: null,
        error_retryable: null,
        created_at: this.now,
        updated_at: this.now,
      }
      this.boldSign.push(row)
      return Promise.resolve([row])
    }
    // error upsert (on conflict signature_request_id do update)
    if (t.includes('insert into bold_sign_request') && t.includes('on conflict (signature_request_id)')) {
      const existing = this.boldSign.find((r) => r.signature_request_id === p[0])
      if (existing) {
        existing.status = 'error'
        existing.last_error = p[1]
        existing.error_retryable = p[2]
        existing.updated_at = this.now
        return Promise.resolve([])
      }
      this.boldSign.push({
        signature_request_id: p[0],
        envelope_id: null,
        document_ids: [],
        status: 'error',
        last_error: p[1],
        error_retryable: p[2],
        created_at: this.now,
        updated_at: this.now,
      })
      return Promise.resolve([])
    }
    if (t.includes('update bold_sign_request set status')) {
      const r = this.boldSign.find((x) => x.signature_request_id === p[2])
      if (r) {
        r.status = p[0]
        r.document_ids = Array.isArray(p[1]) ? [...p[1]] : []
        r.updated_at = this.now
      }
      return Promise.resolve([])
    }
    if (t.includes('from bold_sign_request')) {
      if (t.includes('where signature_request_id')) {
        return Promise.resolve(this.boldSign.filter((r) => r.signature_request_id === p[0]))
      }
      if (t.includes('where envelope_id')) {
        return Promise.resolve(this.boldSign.filter((r) => r.envelope_id === p[0]))
      }
    }

    // ---- bold_sign_webhook_event (provider table — DOC-04) ----
    if (t.includes('insert into bold_sign_webhook_event')) {
      const dup = this.webhookEvents.some((e) => e.provider_event_id === p[0])
      if (dup) return Promise.resolve([])
      this.webhookEvents.push({
        id: `we-${this.webhookEvents.length + 1}`,
        provider_event_id: p[0],
        envelope_id: p[1],
        signature_request_id: p[2],
        provider_event_type: p[3],
        neutral_event: p[4],
        payload: typeof p[5] === 'string' ? JSON.parse(p[5]) : p[5],
        processed_at: null,
        created_at: this.now,
      })
      return Promise.resolve([{ id: `we-${this.webhookEvents.length}` }])
    }
    if (t.includes('from bold_sign_webhook_event')) {
      return Promise.resolve(this.webhookEvents.filter((e) => e.provider_event_id === p[0]))
    }

    throw new Error(`FAKE_UNHANDLED: ${t}`)
  }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

function testConfig(server: FakeBoldSignServer, overrides: Partial<BoldSignConfig> = {}): BoldSignConfig {
  return {
    apiKey: server.apiKey,
    baseUrl: server.baseUrl,
    templateId: 'tpl-1',
    webhookSecret: server.secret,
    timeoutMs: 500,
    maxAttempts: 3,
    retryBaseDelayMs: 5,
    retryMaxDelayMs: 10,
    webhookToleranceSeconds: 300,
    ...overrides,
  }
}

function makeDispatcher(db: FakeDb) {
  return new CommandDispatcherImpl({
    registry: createCommandRegistry(),
    receipts: new PostgresCommandReceiptRepository(),
    run: db.runner,
    now: FIXED_NOW,
  })
}

function makeProvider(
  db: FakeDb,
  server: FakeBoldSignServer,
  overrides: Partial<BoldSignConfig> = {},
  clientDeps: BoldSignClientDeps = {},
): BoldSignSignatureProvider {
  const config = testConfig(server, overrides)
  return new BoldSignSignatureProvider({
    config,
    client: new BoldSignClient(config, { sleep: async () => {}, ...clientDeps }),
    execute: db.tx,
    run: db.runner,
    now: FIXED_NOW,
  })
}

function makeApp(db: FakeDb, provider: BoldSignSignatureProvider) {
  return new SignatureApplication({ dispatcher: makeDispatcher(db), provider, now: FIXED_NOW })
}

function seedDocument(db: FakeDb, overrides: Row = {}): string {
  const id = overrides.id ?? 'doc-1'
  db.documents.push({
    id,
    deal_id: 'deal-1',
    document_type: 'agreement',
    title: 'Purchase Agreement',
    state: 'ready',
    source: 'generated',
    media_id: 'media-1',
    ...overrides,
  })
  // Default unsigned PDF media so direct-PDF send can load the bytes.
  const mediaId = db.documents.find((d) => d.id === id)!.media_id
  db.media.push({
    id: mediaId,
    file_data: overrides.file_data ?? new Uint8Array([37, 80, 68, 70, 10, 13, 10]), // "%PDF"
    filename: overrides.filename ?? 'purchase-agreement.pdf',
    mime_type: overrides.mime_type ?? 'application/pdf',
  })
  return id
}

/** Seed an unsigned PDF media row referenced by a transaction document. */
function seedMedia(db: FakeDb, overrides: Row = {}): string {
  const id = overrides.id ?? 'media-1'
  db.media.push({
    id,
    file_data: overrides.file_data ?? new Uint8Array([37, 80, 68, 70, 10, 13, 10]), // "%PDF"
    filename: overrides.filename ?? 'purchase-agreement.pdf',
    mime_type: overrides.mime_type ?? 'application/pdf',
  })
  return id
}

const RECIPIENTS: SignatureRecipient[] = [
  { role: 'signer', name: 'Buyer One', email: 'buyer1@example.com', order: 1 },
  { role: 'signer', name: 'Buyer Two', email: 'buyer2@example.com', order: 2 },
]

/** Build a raw BoldSign webhook body (the EXACT bytes that get signed). */
function webhookBody(
  eventId: string,
  eventType: string,
  documentId: string,
  documentStatus: string | null,
): string {
  return JSON.stringify({
    event: { id: eventId, created: NOW_SEC, eventType, environment: 'Test' },
    data: { documentId, status: documentStatus },
  })
}

// ---------------------------------------------------------------------------
// 1. Config — credentials from env only, fail closed, never logged
// ---------------------------------------------------------------------------

test('config: reads required keys from env; baseUrl normalized; never echoes values', () => {
  const env = {
    BOLDSIGN_API_KEY: 'key-123',
    BOLDSIGN_BASE_URL: 'https://api.boldsign.com/',
    BOLDSIGN_TEMPLATE_ID: 'tpl-999',
    BOLDSIGN_WEBHOOK_SECRET: 'whsec-xyz',
  }
  const cfg = loadBoldSignConfig(env)
  assert.equal(cfg.apiKey, 'key-123')
  assert.equal(cfg.baseUrl, 'https://api.boldsign.com')
  assert.equal(cfg.templateId, 'tpl-999')
  assert.equal(cfg.webhookSecret, 'whsec-xyz')
  // tuning defaults
  assert.equal(cfg.maxAttempts, 3)
  assert.equal(cfg.timeoutMs, 10_000)
  assert.equal(cfg.webhookToleranceSeconds, 300)
})

test('config: missing credentials fail closed naming the keys, never the values', () => {
  assert.throws(() => loadBoldSignConfig({}), /BOLDSIGN_API_KEY, BOLDSIGN_BASE_URL, BOLDSIGN_WEBHOOK_SECRET/)
  // BOLDSIGN_TEMPLATE_ID is optional — only the three required keys can fail.
  assert.throws(() => loadBoldSignConfig({ BOLDSIGN_API_KEY: 'k', BOLDSIGN_BASE_URL: 'https://x', BOLDSIGN_TEMPLATE_ID: 't' }), /BOLDSIGN_WEBHOOK_SECRET/)
  // blank values are treated as missing
  assert.throws(
    () => loadBoldSignConfig({ BOLDSIGN_API_KEY: ' ', BOLDSIGN_BASE_URL: 'https://x', BOLDSIGN_TEMPLATE_ID: 't', BOLDSIGN_WEBHOOK_SECRET: 's' }),
    /BOLDSIGN_API_KEY/,
  )
  // the error message must never contain a secret VALUE
  try {
    loadBoldSignConfig({ BOLDSIGN_API_KEY: 'super-secret-value' })
    assert.fail('expected loadBoldSignConfig to throw')
  } catch (err) {
    assert.ok(!String(err).includes('super-secret-value'), 'error must not echo secret values')
  }
})

// ---------------------------------------------------------------------------
// 2. Status mapping — BoldSign document statuses -> neutral, fail closed
// ---------------------------------------------------------------------------

test('seam status mapping: BoldSign document statuses -> neutral; unknown fails closed to error', () => {
  assert.equal(mapProviderStatus('bold-sign', 'InProgress'), 'sent')
  assert.equal(mapProviderStatus('bold-sign', 'Completed'), 'completed')
  assert.equal(mapProviderStatus('bold-sign', 'Declined'), 'declined')
  assert.equal(mapProviderStatus('bold-sign', 'Expired'), 'expired')
  assert.equal(mapProviderStatus('bold-sign', 'Revoked'), 'voided')
  assert.equal(mapProviderStatus('bold-sign', 'Draft'), 'requested')
  assert.equal(mapProviderStatus('bold-sign', 'Scheduled'), 'requested')
  // unknown status / unknown provider never cast to a false success
  assert.equal(mapProviderStatus('bold-sign', 'BogusStatus'), 'error')
  assert.equal(mapProviderStatus('other-provider', 'InProgress'), 'error')
  // the vocabulary is exported and total over the documented enum
  assert.equal(BOLD_SIGN_DOCUMENT_STATUSES.length, 7)
})

test('seam status mapping: BoldSign webhook events normalize onto neutral', () => {
  assert.equal(mapBoldSignWebhookEvent('Sent', 'InProgress'), 'sent')
  assert.equal(mapBoldSignWebhookEvent('Viewed', 'InProgress'), 'viewed')
  assert.equal(mapBoldSignWebhookEvent('Signed', 'InProgress'), 'signed')
  assert.equal(mapBoldSignWebhookEvent('Completed', 'Completed'), 'completed')
  assert.equal(mapBoldSignWebhookEvent('Declined', 'Declined'), 'declined')
  assert.equal(mapBoldSignWebhookEvent('Revoked', 'Revoked'), 'voided')
  assert.equal(mapBoldSignWebhookEvent('Expired', 'Expired'), 'expired')
  assert.equal(mapBoldSignWebhookEvent('SendFailed', 'InProgress'), 'error')
  // non-lifecycle events fall back to the document status (safe no-op)
  assert.equal(mapBoldSignWebhookEvent('Reminder', 'InProgress'), 'sent')
  assert.equal(mapBoldSignWebhookEvent('Reassigned', 'InProgress'), 'sent')
  // unmappable payload fails closed
  assert.throws(() => mapBoldSignWebhookEvent('Reminder', 'BogusStatus'), /no neutral lifecycle mapping/)
})

// ---------------------------------------------------------------------------
// 3. Webhook signature verification (HMAC, constant-time, replay tolerance)
// ---------------------------------------------------------------------------

test('webhook signature: valid signature verifies; tampering/secret/stale/malformed reject', () => {
  const secret = 'webhook-secret'
  const body = webhookBody('evt-1', 'Completed', 'env-1', 'Completed')
  const header = signBoldSignWebhook(body, secret, NOW_SEC)
  assert.doesNotThrow(() => verifyBoldSignWebhookSignature(body, header, secret, NOW_SEC, 300))

  // tampered body (raw bytes changed)
  assert.throws(() => verifyBoldSignWebhookSignature(`${body} `, header, secret, NOW_SEC, 300), /invalid/)
  assert.throws(() => verifyBoldSignWebhookSignature(body.replace('Completed', 'Declined'), header, secret, NOW_SEC, 300), /invalid/)

  // wrong secret
  assert.throws(() => verifyBoldSignWebhookSignature(body, header, 'other-secret', NOW_SEC, 300), /invalid/)

  // stale timestamp (replay protection)
  const stale = signBoldSignWebhook(body, secret, NOW_SEC - 10_000)
  assert.throws(() => verifyBoldSignWebhookSignature(body, stale, secret, NOW_SEC, 300), /tolerance/)

  // malformed headers
  assert.throws(() => verifyBoldSignWebhookSignature(body, 'garbage', secret, NOW_SEC, 300), /malformed/)
  assert.throws(() => verifyBoldSignWebhookSignature(body, 's0=deadbeef', secret, NOW_SEC, 300), /malformed/)

  // non-hex signature never matches
  assert.throws(() => verifyBoldSignWebhookSignature(body, `t=${NOW_SEC}, s0=not-hex`, secret, NOW_SEC, 300), /invalid/)
})

test('webhook payload normalization: extracts event id/type + envelope id; fails closed on malformed', () => {
  const body = webhookBody('evt-1', 'Signed', 'env-1', 'InProgress')
  const normalized = parseBoldSignWebhookPayload(body)
  assert.equal(normalized.providerEventId, 'evt-1')
  assert.equal(normalized.eventType, 'Signed')
  assert.equal(normalized.envelopeId, 'env-1')
  assert.equal(normalized.documentStatus, 'InProgress')

  assert.throws(() => parseBoldSignWebhookPayload('not json'), /not valid JSON/)
  assert.throws(() => parseBoldSignWebhookPayload('{"event":{"eventType":"Signed"}}'), /event\.id/)
  assert.throws(() => parseBoldSignWebhookPayload('{"event":{"id":"e1"}}'), /event\.eventType/)
  assert.throws(() => parseBoldSignWebhookPayload('{"event":{"id":"e1","eventType":"Signed"}}'), /data\.documentId/)
})

// ---------------------------------------------------------------------------
// 4. Typed client against the fake BoldSign server
// ---------------------------------------------------------------------------

test('client: send/properties/revoke round-trip with X-API-KEY auth', async () => {
  const server = await FakeBoldSignServer.start()
  try {
    const client = new BoldSignClient(testConfig(server), { sleep: async () => {} })
    const created = await client.sendEnvelopeFromTemplate({
      templateId: 'tpl-1',
      title: 'Signature request',
      message: 'Please sign',
      roles: [
        { roleIndex: 1, signerName: 'Buyer One', signerEmail: 'buyer1@example.com', signerType: 'Signer' },
        { roleIndex: 2, signerName: 'Buyer Two', signerEmail: 'buyer2@example.com', signerType: 'Signer' },
      ],
    })
    assert.ok(created.documentId.startsWith('env-'))

    const props = await client.getDocumentProperties(created.documentId)
    assert.equal(props.status, 'InProgress')
    assert.deepEqual(props.fileIds, [`file-${created.documentId}`])

    await client.revokeDocument(created.documentId)
    const after = await client.getDocumentProperties(created.documentId)
    assert.equal(after.status, 'Revoked')

    // auth header was sent on every request
    const sendReq = server.requests.find((r) => r.path === '/v1/template/send')
    assert.equal(sendReq?.headers['x-api-key'], 'test-api-key')
    const sendBody = JSON.parse(sendReq!.raw)
    assert.equal(sendBody.roles[0].roleIndex, 1)
    assert.equal(sendBody.roles[0].signerEmail, 'buyer1@example.com')
    assert.equal(sendBody.roles[1].signerType, 'Signer')
  } finally {
    await server.stop()
  }
})

test('client: transient 500 retried with capped exponential backoff; 4xx fails immediately', async () => {
  const server = await FakeBoldSignServer.start()
  try {
    server.failNext('POST', '/v1/template/send', 500, 3)
    const delays: number[] = []
    const client = new BoldSignClient(
      testConfig(server, { maxAttempts: 3, retryBaseDelayMs: 100, retryMaxDelayMs: 400 }),
      { sleep: async (ms) => delays.push(ms) },
    )
    await assert.rejects(
      () => client.sendEnvelopeFromTemplate({
        templateId: 'tpl-1', title: null, message: null,
        roles: [{ roleIndex: 1, signerName: 'A', signerEmail: 'a@example.com', signerType: 'Signer' }],
      }),
      /HTTP 500/,
    )
    assert.equal(server.requestCount('POST', '/v1/template/send'), 3, 'transient 500 retried up to maxAttempts')
    assert.deepEqual(delays, [100, 200], 'exponential backoff, capped')

    // a 4xx is NOT retried
    server.failNext('POST', '/v1/template/send', 422, 1)
    const before = server.requestCount('POST', '/v1/template/send')
    await assert.rejects(
      () => client.sendEnvelopeFromTemplate({
        templateId: 'tpl-1', title: null, message: null,
        roles: [{ roleIndex: 1, signerName: 'A', signerEmail: 'a@example.com', signerType: 'Signer' }],
      }),
      /HTTP 422/,
    )
    assert.equal(server.requestCount('POST', '/v1/template/send'), before + 1, '4xx is not retried')
  } finally {
    await server.stop()
  }
})

test('client: timeout aborts and is classified retryable', async () => {
  const server = await FakeBoldSignServer.start()
  try {
    // both attempts hang so the client times out and never recovers
    server.hangNext('POST', '/v1/template/send', 2)
    const client = new BoldSignClient(
      testConfig(server, { timeoutMs: 60, maxAttempts: 2 }),
      { sleep: async () => {} },
    )
    await assert.rejects(
      () => client.sendEnvelopeFromTemplate({
        templateId: 'tpl-1', title: null, message: null,
        roles: [{ roleIndex: 1, signerName: 'A', signerEmail: 'a@example.com', signerType: 'Signer' }],
      }),
      /timed out/,
    )
    assert.equal(server.requestCount('POST', '/v1/template/send'), 2, 'timeout retried once (2 attempts)')
  } finally {
    await server.stop()
  }
})

test('errors: transient classification for statuses and network failures', () => {
  for (const status of [408, 429, 500, 502, 503, 504]) {
    assert.equal(isTransientHttpStatus(status), true, `HTTP ${status} transient`)
  }
  for (const status of [400, 401, 403, 404, 422]) {
    assert.equal(isTransientHttpStatus(status), false, `HTTP ${status} non-transient`)
  }
  const timeout = new Error('timeout')
  timeout.name = 'TimeoutError'
  assert.equal(classifyBoldSignError(timeout).retryable, true)
  const network = new Error('fetch failed')
  network.name = 'TypeError'
  assert.equal(classifyBoldSignError(network).retryable, true)
  assert.equal(classifyBoldSignError(new Error('boom')).retryable, false, 'unknown errors fail closed non-retryable')
})

// ---------------------------------------------------------------------------
// 5. Adapter — send/status/cancel against the fake server + provider store
// ---------------------------------------------------------------------------

test('adapter: send creates ONE envelope, persists provider ids in bold_sign_request only', async () => {
  const db = new FakeDb()
  seedDocument(db)
  seedMedia(db)
  const server = await FakeBoldSignServer.start()
  try {
    const provider = makeProvider(db, server)
    const result = await provider.send({
      signatureRequestId: 'sig-1',
      transactionDocumentId: 'doc-1',
      recipients: RECIPIENTS,
      message: 'Please sign',
    })
    assert.equal(result.ok, true)
    assert.equal(result.providerStatus, 'InProgress')

    assert.equal(server.envelopes.size, 1, 'exactly one envelope at the provider')
    const row = await getBoldSignRequestBySignatureRequestId('sig-1', db.tx)
    assert.ok(row)
    assert.ok(row.envelopeId)
    assert.equal(row.documentIds.length, 0, 'file ids observed on the first status poll')
    assert.equal(row.status, 'InProgress')

    // provider ids live ONLY in the provider row — never in canonical rows
    const envelopeId = row.envelopeId
    assert.ok(!JSON.stringify(db.requests).includes(envelopeId!))
    assert.ok(!JSON.stringify(db.documents).includes(envelopeId!))
  } finally {
    await server.stop()
  }
})

test('adapter: duplicate send is idempotent — same provider row, no second envelope', async () => {
  const db = new FakeDb()
  seedDocument(db)
  seedMedia(db)
  const server = await FakeBoldSignServer.start()
  try {
    const provider = makeProvider(db, server)
    const first = await provider.send({ signatureRequestId: 'sig-1', transactionDocumentId: 'doc-1', recipients: RECIPIENTS, message: null })
    assert.equal(first.ok, true)
    const before = server.requestCount('POST', '/v1/document/send')

    const second = await provider.send({ signatureRequestId: 'sig-1', transactionDocumentId: 'doc-1', recipients: RECIPIENTS, message: null })
    assert.equal(second.ok, true)
    assert.equal(second.providerStatus, 'InProgress')
    assert.equal(server.requestCount('POST', '/v1/document/send'), before, 'no second envelope request')
    assert.equal(server.envelopes.size, 1)
    assert.equal(db.boldSign.length, 1, 'one provider row')
  } finally {
    await server.stop()
  }
})

test('adapter: send failure (500) maps to error with retryable last_error; retries capped', async () => {
  const db = new FakeDb()
  seedDocument(db)
  seedMedia(db)
  const server = await FakeBoldSignServer.start()
  try {
    server.failNext('POST', '/v1/document/send', 500, 3)
    const provider = makeProvider(db, server, { maxAttempts: 3 })
    const result = await provider.send({ signatureRequestId: 'sig-1', transactionDocumentId: 'doc-1', recipients: RECIPIENTS, message: null })
    assert.equal(result.ok, false)
    assert.equal(result.providerStatus, 'error')
    assert.match(result.error ?? '', /HTTP 500/)
    assert.equal(server.requestCount('POST', '/v1/document/send'), 3, 'transient send failure retried up to the cap')

    const row = await getBoldSignRequestBySignatureRequestId('sig-1', db.tx)
    assert.equal(row?.status, 'error')
    assert.equal(row?.envelopeId, null, 'no envelope was created')
    assert.equal(row?.errorRetryable, true, 'retryable classification observable')
    assert.match(row?.lastError ?? '', /HTTP 500/)
  } finally {
    await server.stop()
  }
})

test('adapter: send failure (422) is non-retryable, single attempt, last_error classified', async () => {
  const db = new FakeDb()
  seedDocument(db)
  seedMedia(db)
  const server = await FakeBoldSignServer.start()
  try {
    server.failNext('POST', '/v1/document/send', 422, 1)
    const provider = makeProvider(db, server, { maxAttempts: 3 })
    const result = await provider.send({ signatureRequestId: 'sig-1', transactionDocumentId: 'doc-1', recipients: RECIPIENTS, message: null })
    assert.equal(result.ok, false)
    assert.equal(result.providerStatus, 'error')
    assert.equal(server.requestCount('POST', '/v1/document/send'), 1, '4xx is never retried')
    const row = await getBoldSignRequestBySignatureRequestId('sig-1', db.tx)
    assert.equal(row?.errorRetryable, false)
  } finally {
    await server.stop()
  }
})

test('adapter: status polls the provider, caches RAW status + document ids, maps to neutral', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const server = await FakeBoldSignServer.start()
  try {
    const provider = makeProvider(db, server)
    await provider.send({ signatureRequestId: 'sig-1', transactionDocumentId: 'doc-1', recipients: RECIPIENTS, message: null })
    const row = await getBoldSignRequestBySignatureRequestId('sig-1', db.tx)

    const inProgress = await provider.status('sig-1')
    assert.equal(inProgress.status, 'sent', 'InProgress mapped to neutral sent at the seam')
    let cached = await getBoldSignRequestBySignatureRequestId('sig-1', db.tx)
    assert.deepEqual(cached?.documentIds, [`file-${row?.envelopeId}`], 'provider document ids observed')

    server.setEnvelopeStatus(row!.envelopeId!, 'Completed')
    const completed = await provider.status('sig-1')
    assert.equal(completed.status, 'completed')
    cached = await getBoldSignRequestBySignatureRequestId('sig-1', db.tx)
    assert.equal(cached?.status, 'Completed', 'raw status cached in the provider table')

    // unknown request fails closed to neutral error
    assert.equal((await provider.status('sig-unknown')).status, 'error')
  } finally {
    await server.stop()
  }
})

test('adapter: cancel revokes at the provider and caches Revoked', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const server = await FakeBoldSignServer.start()
  try {
    const provider = makeProvider(db, server)
    await provider.send({ signatureRequestId: 'sig-1', transactionDocumentId: 'doc-1', recipients: RECIPIENTS, message: null })
    const row = await getBoldSignRequestBySignatureRequestId('sig-1', db.tx)

    const cancelled = await provider.cancel('sig-1')
    assert.equal(cancelled.ok, true)
    assert.equal(server.envelopes.get(row!.envelopeId!)?.status, 'Revoked')
    const cached = await getBoldSignRequestBySignatureRequestId('sig-1', db.tx)
    assert.equal(cached?.status, 'Revoked')

    // cancel for a request with no envelope fails cleanly
    const noEnvelope = await provider.cancel('sig-unknown')
    assert.equal(noEnvelope.ok, false)
  } finally {
    await server.stop()
  }
})

// ---------------------------------------------------------------------------
// 6. Adapter — webhook verification, normalization, replay dedupe
// ---------------------------------------------------------------------------

test('adapter: verifyWebhook verifies HMAC, normalizes to neutral, resolves the neutral request id', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const server = await FakeBoldSignServer.start()
  try {
    const provider = makeProvider(db, server)
    await provider.send({ signatureRequestId: 'sig-1', transactionDocumentId: 'doc-1', recipients: RECIPIENTS, message: null })
    const row = await getBoldSignRequestBySignatureRequestId('sig-1', db.tx)
    const envelopeId = row!.envelopeId!

    const body = webhookBody('evt-signed', 'Signed', envelopeId, 'InProgress')
    const header = server.signWebhook(body)
    const verification = await provider.verifyWebhook(body, header)
    assert.equal(verification.event, 'signed')
    assert.equal(verification.signatureRequestId, 'sig-1', 'neutral request id resolved through the provider table')
  } finally {
    await server.stop()
  }
})

test('adapter: webhook replays dedupe by provider event id (unique key)', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const server = await FakeBoldSignServer.start()
  try {
    const provider = makeProvider(db, server)
    await provider.send({ signatureRequestId: 'sig-1', transactionDocumentId: 'doc-1', recipients: RECIPIENTS, message: null })
    const row = await getBoldSignRequestBySignatureRequestId('sig-1', db.tx)
    const envelopeId = row!.envelopeId!

    const body = webhookBody('evt-completed', 'Completed', envelopeId, 'Completed')
    const header = server.signWebhook(body)
    const first = await provider.verifyWebhook(body, header)
    assert.equal(first.event, 'completed')
    const replay = await provider.verifyWebhook(body, header)
    assert.equal(replay.event, 'completed', 'a replay returns the same neutral result')

    const events = db.webhookEvents.filter((e) => e.provider_event_id === 'evt-completed')
    assert.equal(events.length, 1, 'only ONE webhook event receipt despite the replay')
    const stored = await getBoldSignWebhookEventByProviderEventId('evt-completed', db.tx)
    assert.equal(stored?.neutralEvent, 'completed')
    assert.equal(stored?.signatureRequestId, 'sig-1')
  } finally {
    await server.stop()
  }
})

// DOC-05 — the adapter resolves its own envelope through the provider table
// and downloads the signed bytes (base64 per BoldSign's documented download
// shape); provider ids never cross the seam.
test('adapter: downloadSignedArtifact fetches the signed bytes via the provider table', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const server = await FakeBoldSignServer.start()
  try {
    const provider = makeProvider(db, server)
    await provider.send({ signatureRequestId: 'sig-1', transactionDocumentId: 'doc-1', recipients: RECIPIENTS, message: null })
    const row = await getBoldSignRequestBySignatureRequestId('sig-1', db.tx)
    const envelopeId = row!.envelopeId!

    const download = await provider.downloadSignedArtifact('sig-1')
    assert.equal(Buffer.from(download.bytes).equals(Buffer.from(`signed:${envelopeId}:pdf`, 'utf8')), true)
    assert.equal(download.filename, `${envelopeId}-signed.pdf`)
    assert.equal(download.mimeType, 'application/pdf')

    // The download hits the provider for the envelope, and the provider table
    // is only READ (no provider state written by the download).
    assert.ok(server.requestCount('GET', '/v1/document/download') >= 1)
    assert.equal(server.requests.find((r) => r.path === '/v1/document/download')?.query.get('documentId'), envelopeId)

    // Unknown requests fail closed.
    await assert.rejects(() => provider.downloadSignedArtifact('sig-unknown'), /no envelope exists/)
  } finally {
    await server.stop()
  }
})

test('adapter: verifyWebhook rejects forged signatures, stale timestamps, unknown envelopes, non-raw payloads', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const server = await FakeBoldSignServer.start()
  try {
    const provider = makeProvider(db, server)
    await provider.send({ signatureRequestId: 'sig-1', transactionDocumentId: 'doc-1', recipients: RECIPIENTS, message: null })
    const row = await getBoldSignRequestBySignatureRequestId('sig-1', db.tx)
    const envelopeId = row!.envelopeId!

    const body = webhookBody('evt-1', 'Completed', envelopeId, 'Completed')

    // forged signature (valid timestamp, wrong HMAC — the tolerance check
    // passes, so the HMAC mismatch is what must reject)
    await assert.rejects(() => provider.verifyWebhook(body, `t=${NOW_SEC}, s0=deadbeef`), /invalid/)
    // wrong secret
    await assert.rejects(
      () => provider.verifyWebhook(body, server.signWebhook(body, NOW_SEC, 'wrong-secret')),
      /invalid/,
    )
    // stale timestamp
    await assert.rejects(
      () => provider.verifyWebhook(body, server.signWebhook(body, NOW_SEC - 10_000)),
      /tolerance/,
    )
    // unknown envelope
    const unknownBody = webhookBody('evt-2', 'Completed', 'env-unknown', 'Completed')
    await assert.rejects(
      () => provider.verifyWebhook(unknownBody, server.signWebhook(unknownBody)),
      /unknown envelope/,
    )
    // payload must be the RAW body string (HMAC over raw bytes)
    await assert.rejects(
      () => provider.verifyWebhook(JSON.parse(body), server.signWebhook(body)),
      /raw request body string/,
    )
  } finally {
    await server.stop()
  }
})

// ---------------------------------------------------------------------------
// 7. End-to-end through the DOC-03 seam (real adapter, fake server)
// ---------------------------------------------------------------------------

test('e2e: send dispatches to BoldSign; webhook lifecycle completes the request; replay dedupes; provider ids stay confined', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const server = await FakeBoldSignServer.start()
  try {
    const provider = makeProvider(db, server)
    const app = makeApp(db, provider)

    const sent = await app.send(
      { transactionDocumentId: 'doc-1', recipients: RECIPIENTS, message: 'Please sign the agreement' },
      { correlationId: 'wf-1' },
    )
    assert.equal(sent.outcome, 'success')
    const request = (sent.value as { signatureRequest: SignatureRequest }).signatureRequest
    assert.equal(request.status, 'sent', 'provider confirmation mapped to neutral sent')
    assert.equal(sent.emittedEvents.length, 1)
    assert.equal(sent.emittedEvents[0].eventType, 'SIGNATURE_REQUEST_SENT')

    // exactly ONE canonical request, ONE provider row, ONE envelope
    assert.equal(db.requests.length, 1)
    assert.equal(db.boldSign.length, 1)
    assert.equal(server.envelopes.size, 1)
    const providerRow = db.boldSign[0]
    assert.equal(providerRow.signature_request_id, request.id)
    assert.ok(providerRow.envelope_id)
    // provider ids never appear in canonical rows
    assert.ok(!JSON.stringify(db.requests).includes(providerRow.envelope_id))
    assert.ok(!JSON.stringify(db.documents).includes(providerRow.envelope_id))

    // webhook lifecycle: viewed -> signed -> completed
    const envelopeId = providerRow.envelope_id
    for (const [eventId, eventType, status] of [
      ['evt-viewed', 'Viewed', 'InProgress'],
      ['evt-signed', 'Signed', 'InProgress'],
      ['evt-completed', 'Completed', 'Completed'],
    ]) {
      const body = webhookBody(eventId, eventType, envelopeId, status)
      const outcome = await app.handleWebhook(body, server.signWebhook(body), { correlationId: 'wf-1' })
      assert.equal(outcome.result.outcome, 'success')
    }
    const final = db.requests.find((r) => r.id === request.id)!
    assert.equal(final.status, 'completed', 'provider webhook state normalized onto the neutral model')

    // the neutral COMPLETED event was emitted exactly once by the canonical command
    const completedBody = webhookBody('evt-completed', 'Completed', envelopeId, 'Completed')
    const completedOutcome = await app.handleWebhook(
      completedBody,
      server.signWebhook(completedBody),
      { correlationId: 'wf-1' },
    )
    assert.equal(completedOutcome.result.outcome, 'success')
    assert.equal(completedOutcome.result.emittedEvents.length, 0, 'no duplicate event on webhook replay')
    assert.equal(db.webhookEvents.filter((e) => e.provider_event_id === 'evt-completed').length, 1)

    // transaction_document is untouched by intermediate provider state
    const doc = db.documents.find((d) => d.id === 'doc-1')!
    assert.equal(doc.state, 'ready')
    assert.equal(doc.signed_at, undefined, 'signed_at is a DOC-05 reconciliation outcome, not set here')
    assert.equal(doc.signed_media_id, undefined, 'signed_media_id is a DOC-05 reconciliation outcome, not set here')
  } finally {
    await server.stop()
  }
})

test('e2e: duplicate send through the router never creates a second envelope', async () => {
  const db = new FakeDb()
  seedDocument(db)
  seedMedia(db)
  const server = await FakeBoldSignServer.start()
  try {
    const provider = makeProvider(db, server)
    const app = makeApp(db, provider)

    const first = await app.send({ transactionDocumentId: 'doc-1', recipients: RECIPIENTS }, { commandId: 'send-1' })
    assert.equal(first.outcome, 'success')
    const second = await app.send({ transactionDocumentId: 'doc-1', recipients: RECIPIENTS }, { commandId: 'send-2' })
    assert.equal(second.outcome, 'success')

    assert.equal(db.requests.length, 1, 'one canonical request')
    assert.equal(db.boldSign.length, 1, 'one provider row')
    assert.equal(server.requestCount('POST', '/v1/document/send'), 1, 'one envelope request — the adapter is idempotent')
    assert.equal(server.envelopes.size, 1)
  } finally {
    await server.stop()
  }
})

test('e2e: a provider send failure lands as neutral error; status poll reconciles on recovery', async () => {
  const db = new FakeDb()
  seedDocument(db)
  seedMedia(db)
  const server = await FakeBoldSignServer.start()
  try {
    server.failNext('POST', '/v1/document/send', 500, 3)
    const provider = makeProvider(db, server, { maxAttempts: 3 })
    const app = makeApp(db, provider)

    const failed = await app.send(
      { transactionDocumentId: 'doc-1', recipients: RECIPIENTS },
      { correlationId: 'wf-err' },
    )
    assert.equal(failed.outcome, 'success')
    const failedRequest = (failed.value as { signatureRequest: SignatureRequest }).signatureRequest
    assert.equal(failedRequest.status, 'error', 'delivery failure maps to neutral error')
    assert.equal(db.boldSign[0].error_retryable, true, 'retryable classification observable')

    // a NEW send after the terminal error is a NEW canonical request + NEW provider row
    const retry = await app.send(
      { transactionDocumentId: 'doc-1', recipients: RECIPIENTS },
      { correlationId: 'wf-err' },
    )
    assert.equal(retry.outcome, 'success')
    const retryRequest = (retry.value as { signatureRequest: SignatureRequest }).signatureRequest
    assert.equal(retryRequest.status, 'sent')
    assert.notEqual(retryRequest.id, failedRequest.id)
    assert.equal(db.requests.length, 2)
    assert.equal(db.boldSign.length, 2)
  } finally {
    await server.stop()
  }
})

test('e2e: refreshStatus polls BoldSign and the seam emits the neutral completed event on completion', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const server = await FakeBoldSignServer.start()
  try {
    const provider = makeProvider(db, server)
    const app = makeApp(db, provider)

    const sent = await app.send({ transactionDocumentId: 'doc-1', recipients: RECIPIENTS }, { correlationId: 'wf-poll' })
    const request = (sent.value as { signatureRequest: SignatureRequest }).signatureRequest
    const envelopeId = db.boldSign[0].envelope_id

    // The neutral model's completion edge is signed -> completed, so the poll
    // path first observes the intermediate signer states via webhooks, then a
    // status poll of a COMPLETED envelope lands on the legal edge.
    for (const [eventId, eventType] of [['evt-viewed', 'Viewed'], ['evt-signed', 'Signed']]) {
      const body = webhookBody(eventId, eventType, envelopeId, 'InProgress')
      const outcome = await app.handleWebhook(body, server.signWebhook(body), { correlationId: 'wf-poll' })
      assert.equal(outcome.result.outcome, 'success')
    }
    server.setEnvelopeStatus(envelopeId, 'Completed')
    const polled = await app.refreshStatus(request.id, { correlationId: 'wf-poll' })
    assert.equal(polled.outcome, 'success')
    const polledRequest = (polled.value as { signatureRequest: SignatureRequest }).signatureRequest
    assert.equal(polledRequest.status, 'completed')
    assert.equal(polled.emittedEvents.length, 1)
    assert.equal(polled.emittedEvents[0].eventType, 'SIGNATURE_REQUEST_COMPLETED')
    assert.equal(polled.emittedEvents[0].correlationId, 'wf-poll')
    assert.equal(polled.emittedEvents[0].causationId, polled.commandId)
  } finally {
    await server.stop()
  }
})

test('e2e: cancel through the router voids the canonical request and revokes at BoldSign', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const server = await FakeBoldSignServer.start()
  try {
    const provider = makeProvider(db, server)
    const app = makeApp(db, provider)

    const sent = await app.send({ transactionDocumentId: 'doc-1', recipients: RECIPIENTS })
    const request = (sent.value as { signatureRequest: SignatureRequest }).signatureRequest
    const envelopeId = db.boldSign[0].envelope_id

    const cancelled = await app.cancel(request.id, { correlationId: 'wf-cancel' })
    assert.equal(cancelled.outcome, 'success')
    assert.equal(db.requests.find((r) => r.id === request.id)!.status, 'voided')
    assert.equal(cancelled.emittedEvents.length, 1)
    assert.equal(cancelled.emittedEvents[0].eventType, 'SIGNATURE_REQUEST_VOIDED')
    assert.equal(server.envelopes.get(envelopeId)?.status, 'Revoked', 'provider revoke executed after commit')
  } finally {
    await server.stop()
  }
})

test('e2e: canonical reads still work and never expose provider state', async () => {
  const db = new FakeDb()
  seedDocument(db)
  const server = await FakeBoldSignServer.start()
  try {
    const provider = makeProvider(db, server)
    const app = makeApp(db, provider)
    await app.send({ transactionDocumentId: 'doc-1', recipients: RECIPIENTS })
    const requestId = db.requests[0].id

    const byId = await getSignatureRequest(requestId, db.tx)
    assert.equal(byId?.id, requestId)
    assert.deepEqual(
      Object.keys(db.requests[0]).sort(),
      ['created_at', 'created_by_user_id', 'id', 'message', 'status', 'transaction_document_id', 'updated_at'].sort(),
      'canonical signature_request carries zero provider fields',
    )
    const active = await getActiveSignatureRequestForDocument('doc-1', db.tx)
    assert.equal(active?.id, requestId)
    const providerRow = await getBoldSignRequestBySignatureRequestId(requestId, db.tx)
    assert.equal(providerRow?.signatureRequestId, requestId)
  } finally {
    await server.stop()
  }
})
