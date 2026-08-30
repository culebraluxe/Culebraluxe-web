import { resolveBrokerSignatureForIssuance, getBrokerSignatureConfig } from '@/db/broker-signature'
import { sql } from '@/db/client'
import { resolveRequiredSlots, type IssuedExecutionSlot } from '@/lib/agreements/execution'
import { resolveSignatureEnvelopeRecipients } from '@/lib/forms/signature-envelope'
import { getTemplate } from '@/lib/forms/template-registry'
import { prefillFieldValues, emptyDealFacts } from '@/lib/forms/offer-letter-data'
import { renderFormPdf } from '@/lib/forms/pdf'

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

function slots(sellerCount: number): IssuedExecutionSlot[] {
  const result: IssuedExecutionSlot[] = []
  for (let index = 0; index < sellerCount; index += 1) {
    result.push({
      slotId: `SELLER:${index + 1}`,
      role: 'SELLER',
      personId: null,
      name: `Seller ${index + 1}`,
      email: `seller${index + 1}@example.com`,
      required: false,
      order: index,
    })
  }
  result.push({
    slotId: 'SELLER_BROKER:1',
    role: 'SELLER_BROKER',
    personId: null,
    name: 'Lisa Penfield',
    email: null,
    required: false,
    order: sellerCount,
  })
  return result
}

export async function GET() {
  try {
    const template = getTemplate('LISTING-01', 2)
    if (!template) throw new Error('LISTING-01 v2 unavailable')

    const values = prefillFieldValues(template, emptyDealFacts())
    const config = getBrokerSignatureConfig()
    const local = await resolveBrokerSignatureForIssuance(
      {
        template,
        values,
        participants: slots(1),
        actorAppUserId: config.appUserId,
        issuedAt: new Date().toISOString(),
      },
      sql,
      config,
    )
    if (!local.ok || local.signatures.length !== 1) {
      return Response.json(
        { ok: false, stage: 'local-signature', message: local.ok ? 'No local signature resolved' : local.message },
        { status: 500, headers: { 'Cache-Control': 'no-store' } },
      )
    }

    const rendered = await renderFormPdf(template, values, {}, 1, {
      participants: slots(1),
      appliedSignatures: local.signatures,
    })
    const raw = rendered.toString('latin1')
    const imageObjects = (raw.match(/\/Subtype\s*\/Image/g) ?? []).length

    const cases = [1, 2, 4].map((sellerCount) => {
      const required = resolveRequiredSlots('LISTING-01', slots(sellerCount))
      const envelope = resolveSignatureEnvelopeRecipients(required, ['SELLER_BROKER:1'])
      if (!envelope.ok) return { sellerCount, ok: false, error: envelope.error }
      return {
        sellerCount,
        ok: true,
        recipientCount: envelope.recipients.length,
        recipients: envelope.recipients.map((recipient) => ({
          name: recipient.name,
          order: recipient.order,
          executionRole: recipient.executionRole,
          executionSlotId: recipient.executionSlotId,
        })),
      }
    })

    return Response.json(
      {
        ok: imageObjects > 1 && values.brokerName === 'Lisa Penfield' && cases.every((item) => item.ok && item.recipientCount === item.sellerCount),
        templateVersion: template.version,
        brokerName: values.brokerName,
        localSignatureCount: local.signatures.length,
        localSignatureRole: local.signatures[0]?.role,
        localSignatureSlotId: local.signatures[0]?.slotId,
        pdfImageObjects: imageObjects,
        cases,
      },
      { headers: { 'Cache-Control': 'no-store' } },
    )
  } catch (error) {
    return Response.json(
      { ok: false, message: error instanceof Error ? error.message : String(error) },
      { status: 500, headers: { 'Cache-Control': 'no-store' } },
    )
  }
}
