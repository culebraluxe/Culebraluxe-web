import test from 'node:test'
import assert from 'node:assert/strict'

import { SignatureApplication } from '../../lib/signature/application'
import type { SignatureProvider } from '../../lib/signature/provider'
import type { CommandDispatcher, CommandResult } from '../../lib/commands/contracts'
import type { CommandEnvelope } from '../../lib/workflow/contracts'

const NOW = new Date('2026-08-22T00:00:00.000Z')

function result(
  envelope: CommandEnvelope,
  emittedEvents: CommandResult['emittedEvents'] = [],
): CommandResult {
  return {
    commandId: envelope.commandId,
    outcome: 'success',
    emittedEvents,
    aggregateId: envelope.aggregateId,
    message: null,
    replayed: false,
  }
}

class FakeDispatcher implements CommandDispatcher {
  completed = new Set<string>()
  calls: CommandEnvelope[] = []

  async execute(envelope: CommandEnvelope): Promise<CommandResult> {
    this.calls.push(envelope)
    if (envelope.commandType === 'signature.request.status') {
      const input = envelope.input as {
        signatureRequestId: string
        targetStatus: string
      }
      if (input.targetStatus === 'completed' && !this.completed.has(input.signatureRequestId)) {
        this.completed.add(input.signatureRequestId)
        return result(envelope, [{
          eventId: `evt-${input.signatureRequestId}`,
          eventType: 'SIGNATURE_REQUEST_COMPLETED',
          occurredAt: NOW.toISOString(),
          actorAppUserId: null,
          aggregateType: 'signature_request',
          aggregateId: input.signatureRequestId,
          correlationId: envelope.correlationId,
          causationId: envelope.commandId,
          payload: {
            signatureRequestId: input.signatureRequestId,
            transactionDocumentId: 'doc-1',
            status: 'completed',
          },
        }])
      }
    }
    return result(envelope)
  }
}

class FakeProvider implements SignatureProvider {
  readonly name = 'fake'
  webhookEvent: 'sent' | 'viewed' | 'signed' | 'completed' | 'declined' | 'expired' | 'voided' | 'error' = 'completed'
  requestId = 'sig-1'

  async send() {
    return { ok: true as const, providerStatus: 'sent' }
  }
  async status() {
    return { status: this.webhookEvent === 'completed' ? 'completed' as const : 'viewed' as const }
  }
  async cancel() {
    return { ok: true as const }
  }
  async verifyWebhook() {
    return { event: this.webhookEvent, signatureRequestId: this.requestId }
  }
  async downloadSignedArtifact() {
    return {
      bytes: new Uint8Array([37, 80, 68, 70]),
      filename: 'signed.pdf',
      mimeType: 'application/pdf',
    }
  }
  async downloadAuditTrail() {
    return {
      bytes: new Uint8Array([37, 80, 68, 70]),
      filename: 'audit.pdf',
      mimeType: 'application/pdf',
    }
  }
}

function reconciliationResult(
  replayed: boolean,
  commandId: string,
): CommandResult {
  return {
    commandId,
    outcome: 'success',
    emittedEvents: [],
    aggregateId: 'doc-1',
    message: replayed
      ? 'Document already signed; completion treated as replayed.'
      : 'Signed artifact reconciled.',
    replayed: false,
    value: {
      replayed,
      documentId: 'doc-1',
      mediaId: 'media-signed',
      auditMediaId: 'media-audit',
      signedAt: NOW.toISOString(),
      signatureRequestId: 'sig-1',
    },
  }
}

function makeHarness() {
  const dispatcher = new FakeDispatcher()
  const provider = new FakeProvider()
  let completedCalls = 0
  let retryCalls = 0
  const reconciler = {
    async onCompletedEvent(event: { eventId: string }) {
      completedCalls += 1
      return reconciliationResult(false, event.eventId)
    },
    async retryCompletedRequest(_requestId: string, commandId: string) {
      retryCalls += 1
      return reconciliationResult(true, commandId)
    },
  }
  const app = new SignatureApplication({
    dispatcher,
    provider,
    reconciler: reconciler as never,
    now: () => NOW,
  })
  return {
    app,
    dispatcher,
    provider,
    counts: () => ({ completedCalls, retryCalls }),
  }
}

test('completed webhook reconciles the signed artifact after the neutral completion event', async () => {
  const h = makeHarness()
  const outcome = await h.app.handleWebhook({}, 'valid-signature', {
    correlationId: 'wf-1',
  })
  assert.equal(outcome.event, 'completed')
  assert.equal(outcome.result.outcome, 'success')
  assert.equal(outcome.result.emittedEvents.length, 1)
  assert.equal(outcome.reconciliation?.outcome, 'success')
  assert.equal((outcome.reconciliation?.value as { replayed: boolean }).replayed, false)
  assert.deepEqual(h.counts(), { completedCalls: 1, retryCalls: 0 })
})

test('completed webhook re-delivery performs explicit replay recovery without a second completion event', async () => {
  const h = makeHarness()
  const first = await h.app.handleWebhook({}, 'valid-signature', {
    correlationId: 'wf-1',
  })
  assert.equal(first.result.emittedEvents.length, 1)

  const redelivery = await h.app.handleWebhook({}, 'valid-signature', {
    correlationId: 'wf-1',
  })
  assert.equal(redelivery.result.outcome, 'success')
  assert.equal(
    redelivery.result.emittedEvents.length,
    0,
    'status replay never emits a duplicate neutral completion event',
  )
  assert.equal(
    redelivery.reconciliation?.outcome,
    'success',
    'provider-completed observation still invokes the idempotent recovery seam',
  )
  assert.equal(
    (redelivery.reconciliation?.value as { replayed: boolean }).replayed,
    true,
    'already-signed document is returned as explicit replay evidence',
  )
  assert.deepEqual(h.counts(), { completedCalls: 1, retryCalls: 1 })
})

test('non-completed webhook does not invoke signed-artifact reconciliation', async () => {
  const h = makeHarness()
  h.provider.webhookEvent = 'viewed'
  const outcome = await h.app.handleWebhook({}, 'valid-signature')
  assert.equal(outcome.event, 'viewed')
  assert.equal(outcome.result.emittedEvents.length, 0)
  assert.equal(outcome.reconciliation, null)
  assert.deepEqual(h.counts(), { completedCalls: 0, retryCalls: 0 })
})

test('completed status poll retries reconciliation when canonical status is already complete', async () => {
  const h = makeHarness()
  await h.app.handleWebhook({}, 'valid-signature')
  const refreshed = await h.app.refreshStatus('sig-1')
  assert.equal(refreshed.outcome, 'success')
  assert.equal(refreshed.emittedEvents.length, 0)
  assert.deepEqual(h.counts(), { completedCalls: 1, retryCalls: 1 })
})

test('failed completed-artifact recovery is surfaced instead of silently accepting completion', async () => {
  const dispatcher = new FakeDispatcher()
  const provider = new FakeProvider()
  const reconciler = {
    async onCompletedEvent() {
      return {
        commandId: 'reconcile-1',
        outcome: 'conflict',
        emittedEvents: [],
        aggregateId: 'doc-1',
        message: 'signed artifact download failed',
        replayed: false,
      } as CommandResult
    },
    async retryCompletedRequest() {
      throw new Error('should not be called on first completion')
    },
  }
  const app = new SignatureApplication({
    dispatcher,
    provider,
    reconciler: reconciler as never,
    now: () => NOW,
  })
  await assert.rejects(
    () => app.handleWebhook({}, 'valid-signature'),
    /signed artifact download failed/,
  )
})
