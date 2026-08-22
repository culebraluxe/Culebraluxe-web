// ---------------------------------------------------------------------------
// DOC-04 — BoldSign Integration: typed BoldSign API client.
//
// A thin typed wrapper over the BoldSign REST API (developers.boldsign.com).
// The adapter talks to THIS client, never to BoldSign directly; all BoldSign
// wire shapes are confined here (and to the webhook parser). The client:
//   - authenticates with the API key via the `X-API-KEY` header (never
//     logged, never embedded in error messages);
//   - enforces a per-attempt timeout (AbortController);
//   - retries only TRANSIENT failures (network/timeout/408/429/5xx) with
//     capped exponential backoff (maxAttempts, base/cap delays) — no retry
//     storms; non-transient failures fail immediately;
//   - parses JSON responses into the typed shapes the adapter needs.
//
// Endpoints (v1):
//   POST /v1/template/send?templateId=...   -> 201 { documentId }
//   GET  /v1/document/properties?documentId=... -> { documentId, status, files }
//   POST /v1/document/revoke                -> 200 { }
// ---------------------------------------------------------------------------

import type { BoldSignConfig } from './config'
import {
  BoldSignProviderError,
  classifyBoldSignError,
  isTransientHttpStatus,
} from './errors'

export type BoldSignSignerType = 'Signer' | 'Reviewer'

export type BoldSignTemplateRole = {
  roleIndex: number
  signerName: string
  signerEmail: string
  signerType: BoldSignSignerType
}

export type BoldSignSendEnvelopeInput = {
  templateId: string
  title: string | null
  message: string | null
  roles: BoldSignTemplateRole[]
}

export type BoldSignEnvelopeCreated = {
  documentId: string
}

export type BoldSignDocumentProperties = {
  documentId: string
  status: string
  fileIds: string[]
}

export type BoldSignClientDeps = {
  fetchFn?: typeof fetch
  sleep?: (ms: number) => Promise<void>
}

export class BoldSignClient {
  constructor(
    private readonly config: BoldSignConfig,
    private readonly deps: BoldSignClientDeps = {},
  ) {}

  /** Send a signature request from the configured template. Creates a NEW
   *  BoldSign envelope (document). At-least-once note: a retry after a lost
   *  response may create a duplicate envelope at BoldSign; the provider table
   *  backstop (unique envelope_id) and the adapter's idempotent lookup ensure
   *  at most ONE provider row per request — an orphaned envelope is surfaced
   *  through last_error for manual reconciliation. */
  async sendEnvelopeFromTemplate(input: BoldSignSendEnvelopeInput): Promise<BoldSignEnvelopeCreated> {
    const query = new URLSearchParams({ templateId: input.templateId })
    return this.requestWithRetry<BoldSignEnvelopeCreated>({
      method: 'POST',
      path: `/v1/template/send?${query.toString()}`,
      body: JSON.stringify({
        title: input.title,
        message: input.message,
        roles: input.roles,
      }),
      parse: (json) => {
        const created = json as { documentId?: unknown }
        if (typeof created?.documentId !== 'string' || created.documentId.trim() === '') {
          throw new Error('BoldSign template send response is missing documentId.')
        }
        return { documentId: created.documentId }
      },
    })
  }

  /** Fetch the current envelope status + file ids. */
  async getDocumentProperties(documentId: string): Promise<BoldSignDocumentProperties> {
    const query = new URLSearchParams({ documentId })
    return this.requestWithRetry<BoldSignDocumentProperties>({
      method: 'GET',
      path: `/v1/document/properties?${query.toString()}`,
      parse: (json) => {
        const props = json as {
          documentId?: unknown
          status?: unknown
          files?: Array<{ id?: unknown }>
        }
        if (typeof props?.documentId !== 'string' || props.documentId.trim() === '') {
          throw new Error('BoldSign document properties response is missing documentId.')
        }
        if (typeof props.status !== 'string' || props.status.trim() === '') {
          throw new Error('BoldSign document properties response is missing status.')
        }
        const fileIds = Array.isArray(props.files)
          ? props.files
              .map((f) => (typeof f?.id === 'string' ? f.id : null))
              .filter((v): v is string => v !== null)
          : []
        return { documentId: props.documentId, status: props.status, fileIds }
      },
    })
  }

  /** Revoke/cancel an envelope (best-effort). */
  async revokeDocument(documentId: string, reason = 'Signature request cancelled.'): Promise<void> {
    await this.requestWithRetry<unknown>({
      method: 'POST',
      path: '/v1/document/revoke',
      body: JSON.stringify({ documentId, reason }),
      parse: () => undefined,
      acceptEmpty: true,
    })
  }

  // -------------------------------------------------------------------------

  private async requestWithRetry<T>(opts: {
    method: string
    path: string
    body?: string
    parse: (json: unknown) => T
    acceptEmpty?: boolean
  }): Promise<T> {
    const maxAttempts = Math.max(1, this.config.maxAttempts)
    let lastError: unknown
    for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
      try {
        return await this.requestOnce(opts)
      } catch (err) {
        const classified = classifyBoldSignError(err)
        if (!classified.retryable || attempt >= maxAttempts) {
          throw err
        }
        lastError = err
        const delay = Math.min(
          this.config.retryBaseDelayMs * 2 ** (attempt - 1),
          this.config.retryMaxDelayMs,
        )
        if (this.deps.sleep) {
          // eslint-disable-next-line no-await-in-loop
          await this.deps.sleep(delay)
        } else {
          // eslint-disable-next-line no-await-in-loop
          await new Promise((resolve) => setTimeout(resolve, delay))
        }
      }
    }
    throw lastError
  }

  private async requestOnce<T>(opts: {
    method: string
    path: string
    body?: string
    parse: (json: unknown) => T
    acceptEmpty?: boolean
  }): Promise<T> {
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), this.config.timeoutMs)
    const fetchFn = this.deps.fetchFn ?? fetch
    try {
      const response = await fetchFn(`${this.config.baseUrl}${opts.path}`, {
        method: opts.method,
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
          'x-api-key': this.config.apiKey,
        },
        body: opts.body,
        signal: controller.signal,
      })
      const raw = await response.text()
      if (!response.ok) {
        const excerpt = raw.length > 200 ? `${raw.slice(0, 200)}…` : raw
        throw new BoldSignProviderError(
          `BoldSign API ${opts.method} ${opts.path} failed with HTTP ${response.status}: ${excerpt}`,
          response.status,
          isTransientHttpStatus(response.status),
        )
      }
      if (opts.acceptEmpty && raw.trim() === '') return opts.parse(undefined)
      if (raw.trim() === '') {
        throw new Error(`BoldSign API ${opts.method} ${opts.path} returned an empty body.`)
      }
      let json: unknown
      try {
        json = JSON.parse(raw)
      } catch {
        throw new Error(`BoldSign API ${opts.method} ${opts.path} returned non-JSON content.`)
      }
      return opts.parse(json)
    } catch (err) {
      // Re-surface fetch network/timeout failures as retryable, preserving the
      // original error (AbortError/TypeError) so classifyBoldSignError tags
      // them transient.
      if (err instanceof Error && err.name === 'AbortError') {
        const timeoutErr = new Error(`BoldSign API ${opts.method} ${opts.path} timed out after ${this.config.timeoutMs}ms.`)
        timeoutErr.name = 'TimeoutError'
        throw timeoutErr
      }
      throw err
    } finally {
      clearTimeout(timeout)
    }
  }
}
