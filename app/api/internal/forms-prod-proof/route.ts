import { NextResponse } from 'next/server'

import { getFormInstance } from '@/db/document-form-instance'
import { listFormSignerPeople } from '@/db/form-signer'
import { resolveBrokerSignatureForIssuance } from '@/db/broker-signature'
import { sql } from '@/db/client'
import { canonicalizeExecutionParticipants } from '@/lib/agreements/participants'
import { getTemplate } from '@/lib/forms/template-registry'
import { renderFormPdfArtifact } from '@/lib/forms/pdf'

export const dynamic = 'force-dynamic'

const PROOF_TOKEN = 'prod-listing-proof-0830-a91e'

export async function GET(request: Request) {
  const url = new URL(request.url)
  if (url.searchParams.get('token') !== PROOF_TOKEN) {
    return new NextResponse('Not found', { status: 404 })
  }
  const formId = url.searchParams.get('formId')?.trim() || ''
  if (!formId) return new NextResponse('formId required', { status: 400 })

  const form = await getFormInstance(formId, sql)
  if (!form) return new NextResponse('form not found', { status: 404 })
  const template = getTemplate(form.templateId, form.templateVersion)
  if (!template) return new NextResponse('template not found', { status: 404 })

  const values = {
    ...form.fieldValues,
    ...(template.id === 'LISTING-01' ? { brokerName: 'Lisa Penfield' } : {}),
  }
  const people = await listFormSignerPeople(form.id, sql)
  const participants = canonicalizeExecutionParticipants(people)
  const broker = await resolveBrokerSignatureForIssuance(
    {
      template,
      values,
      participants,
      actorAppUserId: form.createdByUserId,
      issuedAt: new Date().toISOString(),
    },
    sql,
    undefined,
    { requireExecutionSlot: false },
  )
  if (!broker.ok) {
    return NextResponse.json({ ok: false, stage: 'broker', message: broker.message }, { status: 500 })
  }

  const artifact = await renderFormPdfArtifact(
    template,
    values,
    form.sections,
    1,
    { participants, appliedSignatures: broker.signatures },
  )

  return NextResponse.json({
    ok: true,
    formId: form.id,
    template: `${template.id} v${template.version}`,
    storedBrokerName: form.fieldValues.brokerName ?? null,
    effectiveBrokerName: values.brokerName ?? null,
    participants: participants.map((p) => ({
      slotId: p.slotId,
      role: p.role,
      name: p.name,
      hasPersonId: Boolean(p.personId),
      hasEmail: Boolean(p.email),
      order: p.order,
    })),
    applied: broker.signatures.map((s) => ({
      role: s.role,
      slotId: s.slotId,
      signerName: s.signerName,
      credentialLine: s.credentialLine,
      imageBytes: s.imageBytes.length,
      initials: 'LP',
      appliedAt: s.appliedAt,
    })),
    renderedEvidence: artifact.appliedSignatures.map((s) => ({
      role: s.role,
      slotId: s.slotId,
      signerName: s.signerName,
      renderedInitials: s.renderedInitials,
      renderedDate: s.renderedDate,
      credentialLine: s.credentialLine,
      pageIndex: s.pageIndex,
      signatureRect: s.signatureRect,
      initialsRect: s.initialsRect,
      dateRect: s.dateRect,
    })),
    pageCount: artifact.pageCount,
    pdfBytes: artifact.bytes.length,
  })
}
