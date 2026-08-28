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
//   POST /v1/document/send                  -> 201 { documentId }
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

/**
 * A signature/form field bound to a signer for DIRECT document send. Matches
 * BoldSign's `FormField` model: `fieldType` (e.g. "Signature"), `pageNumber`
 * (1-based), `bounds` (Rectangle) and optional `isRequired`. Provider-specific —
 * never crosses the seam.
 */
export type BoldSignDirectBounds = {
  x: number
  y: number
  width: number
  height: number
}

export type BoldSignDirectFormField = {
  fieldType: string
  pageNumber: number
  bounds: BoldSignDirectBounds
  isRequired?: boolean
  /**
   * Font size (points) for typed content. Required for Signature fields: BoldSign
   * defaults to a large size (~48pt) for a typed signature; an explicit modest
   * `fontSize` keeps the rendered signature readable instead of dominating the page.
   */
  fontSize?: number
  /** BoldSign DateSigned display format. */
  dateFormat?: string
}

/**
 * A signer for DIRECT document send. Matches BoldSign's `DocumentSigner` model
 * exactly: `signerOrder` drives signing order (there is no `roleIndex` field)
 * and `formFields` is REQUIRED (a signer with null/empty FormFields is rejected
 * with HTTP 400 "Form fields cannot be null"). The model binds strictly with
 * additionalProperties=false, so only these fields may be sent. Provider-
 * specific — never crosses the seam.
 */
export type BoldSignDirectSigner = {
  name: string
  emailAddress: string
  signerType: BoldSignSignerType
  signerOrder: number
  authenticationType: 'EmailOTP'
  formFields: BoldSignDirectFormField[]
}

export type BoldSignSendDocumentInput = {
  /** The existing unsigned PDF bytes CulebraLuxe already owns. */
  fileBytes: Uint8Array
  filename: string
  mimeType: string
  title: string | null
  message: string | null
  signers: BoldSignDirectSigner[]
  enableSigningOrder: boolean
  completionCcEmails: string[]
}

export type BoldSignEnvelopeCreated = {
  documentId: string
}

export type BoldSignDocumentProperties = {
  documentId: string
  status: string
  fileIds: string[]
}

/** Decoded signed-artifact bytes + storage metadata (DOC-05 download). */
export type BoldSignDocumentDownload = {
  bytes: Uint8Array
  filename: string
  mimeType: string
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

  /**
   * Send an existing PDF DIRECTLY to BoldSign as multipart/form-data
   * (POST /v1/document/send). The PDF bytes are already owned by CulebraLuxe;
   * BoldSign only signs them. No BoldSign template is required. At-least-once
   * note (same as template send): a retry after a lost response may create a
   * duplicate envelope at BoldSign; the provider table backstop (unique
   * envelope_id) and the adapter's idempotent lookup ensure at most ONE provider
   * row per request.
   */
  async sendDocument(input: BoldSignSendDocumentInput): Promise<BoldSignEnvelopeCreated> {
    const form = new FormData()
    form.set(
      'files',
      new Blob([input.fileBytes as BlobPart], { type: input.mimeType }),
      input.filename,
    )
    form.set('title', input.title ?? '')
    form.set('message', input.message ?? '')
    // BoldSign's /v1/document/send binds via ASP.NET [FromForm]: a
    // List<DocumentSigner> is bound from NESTED form fields (signers[i].field),
    // not from a JSON string in a single `signers` field. Sending a JSON string
    // makes ASP.NET reject the whole request with HTTP 400
    // {"Signers":["Value is invalid"]}. Each signer maps to the DocumentSigner
    // model fields (name/emailAddress/signerType/signerOrder).
    input.signers.forEach((signer, index) => {
      const prefix = `signers[${index}]`
      form.append(`${prefix}.name`, signer.name)
      form.append(`${prefix}.emailAddress`, signer.emailAddress)
      form.append(`${prefix}.signerType`, signer.signerType)
      form.append(`${prefix}.signerOrder`, String(signer.signerOrder))
      form.append(`${prefix}.authenticationType`, signer.authenticationType)
      signer.formFields.forEach((field, fieldIndex) => {
        const fp = `${prefix}.formFields[${fieldIndex}]`
        form.append(`${fp}.fieldType`, field.fieldType)
        form.append(`${fp}.pageNumber`, String(field.pageNumber))
        form.append(`${fp}.bounds.x`, String(field.bounds.x))
        form.append(`${fp}.bounds.y`, String(field.bounds.y))
        form.append(`${fp}.bounds.width`, String(field.bounds.width))
        form.append(`${fp}.bounds.height`, String(field.bounds.height))
        if (field.isRequired !== undefined) {
          form.append(`${fp}.isRequired`, field.isRequired ? 'true' : 'false')
        }
        if (field.fontSize !== undefined) {
          form.append(`${fp}.fontSize`, String(field.fontSize))
        }
        if (field.dateFormat) {
          form.append(`${fp}.dateFormat`, field.dateFormat)
        }
      })
    })
    input.completionCcEmails.forEach((email, index) => {
      form.append(`cc[${index}].emailAddress`, email)
    })
    if (input.completionCcEmails.length > 0) {
      form.set('recipientNotificationSettings.completed', 'true')
    }
    if (input.enableSigningOrder) form.set('enableSigningOrder', 'true')

    return this.requestWithRetry<BoldSignEnvelopeCreated>({
      method: 'POST',
      path: '/v1/document/send',
      body: form,
      parse: (json) => {
        const created = json as { documentId?: unknown }
        if (typeof created?.documentId !== 'string' || created.documentId.trim() === '') {
          throw new Error('BoldSign document send response is missing documentId.')
        }
        return { documentId: created.documentId }
      },
    })
  }
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

  /**
   * Download a signed document. BoldSign returns the file content base64-encoded
   * in the JSON body (`{ file: <base64> }` — see developers.boldsign.com,
   * "Download a Document as Base64"); this decodes it to bytes for the DOC-05
   * signed-artifact append. `fileName`/`mimeType` are honored when present and
   * default to deterministic PDF naming otherwise.
   */
  async downloadDocument(documentId: string): Promise<BoldSignDocumentDownload> {
    const query = new URLSearchParams({ documentId })
    return this.requestWithRetry<BoldSignDocumentDownload>({
      method: 'GET',
      path: `/v1/document/download?${query.toString()}`,
      parse: (json) => {
        const body = json as { file?: unknown; fileName?: unknown; mimeType?: unknown }
        if (typeof body?.file !== 'string' || body.file.trim() === '') {
          throw new Error('BoldSign document download response is missing file (base64).')
        }
        const bytes = Buffer.from(body.file, 'base64')
        if (bytes.length === 0) {
          throw new Error('BoldSign document download returned an empty file.')
        }
        return {
          bytes,
          filename:
            typeof body.fileName === 'string' && body.fileName.trim() !== ''
              ? body.fileName
              : `${documentId}-signed.pdf`,
          mimeType:
            typeof body.mimeType === 'string' && body.mimeType.trim() !== ''
              ? body.mimeType
              : 'application/pdf',
        }
      },
    })
  }

  /** Download BoldSign's completed-envelope audit trail PDF. */
  async downloadAuditTrail(documentId: string): Promise<BoldSignDocumentDownload> {
    const query = new URLSearchParams({ documentId })
    const bytes = await this.requestBinaryWithRetry(
      `/v1/document/downloadAuditLog?${query.toString()}`,
    )
    return {
      bytes,
      filename: `${documentId}-audit-trail.pdf`,
      mimeType: 'application/pdf',
    }
  }

  // -------------------------------------------------------------------------

  private async requestWithRetry<T>(opts: {
    method: string
    path: string
    body?: string | FormData
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

  private async requestBinaryWithRetry(path: string): Promise<Uint8Array> {
    const maxAttempts = Math.max(1, this.config.maxAttempts)
    let lastError: unknown
    for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
      const controller = new AbortController()
      const timeout = setTimeout(() => controller.abort(), this.config.timeoutMs)
      try {
        const fetchFn = this.deps.fetchFn ?? fetch
        const response = await fetchFn(`${this.config.baseUrl}${path}`, {
          method: 'GET',
          headers: { accept: 'application/pdf', 'x-api-key': this.config.apiKey },
          signal: controller.signal,
        })
        if (!response.ok) {
          const raw = await response.text()
          const excerpt = raw.length > 200 ? `${raw.slice(0, 200)}…` : raw
          throw new BoldSignProviderError(
            `BoldSign API GET ${path} failed with HTTP ${response.status}: ${excerpt}`,
            response.status,
            isTransientHttpStatus(response.status),
          )
        }
        const bytes = new Uint8Array(await response.arrayBuffer())
        if (bytes.length === 0) throw new Error('BoldSign audit trail download returned an empty file.')
        return bytes
      } catch (error) {
        const classified = classifyBoldSignError(error)
        if (!classified.retryable || attempt >= maxAttempts) throw error
        lastError = error
        const delay = Math.min(
          this.config.retryBaseDelayMs * 2 ** (attempt - 1),
          this.config.retryMaxDelayMs,
        )
        if (this.deps.sleep) await this.deps.sleep(delay)
        else await new Promise((resolve) => setTimeout(resolve, delay))
      } finally {
        clearTimeout(timeout)
      }
    }
    throw lastError
  }

  private async requestOnce<T>(opts: {
    method: string
    path: string
    body?: string | FormData
    parse: (json: unknown) => T
    acceptEmpty?: boolean
  }): Promise<T> {
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), this.config.timeoutMs)
    const fetchFn = this.deps.fetchFn ?? fetch
    try {
      // JSON bodies get the JSON content-type; multipart (FormData) bodies must
      // NOT, so fetch sets the multipart boundary header itself.
      const headers: Record<string, string> = {
        accept: 'application/json',
        'x-api-key': this.config.apiKey,
      }
      if (typeof opts.body === 'string') headers['content-type'] = 'application/json'
      const response = await fetchFn(`${this.config.baseUrl}${opts.path}`, {
        method: opts.method,
        headers,
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
