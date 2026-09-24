import { NextResponse, type NextRequest } from 'next/server'

import { submitWebsiteIntake } from '@/app/actions/website-intake'
import { withApiHandler } from '@/lib/error-capture-seam'

// ---------------------------------------------------------------------------
// THE CONTACT FORM'S TRANSPORT, FOR THE RUST UI.
//
// The Yew contact page cannot call a React server action, so it posts here and this hands the fields to the SAME
// action the TypeScript form called. Nothing is decided here: parsing, the honeypot, validation, idempotency by
// submission id and the CRM write all stay in the one intake pipeline, so there is still one way a lead arrives.
//
// The body is JSON of strings, checked here before it becomes FormData - a value that is not a string is dropped,
// never coerced - and `parseWebsiteIntakeFormData` validates the rest exactly as it did for the React form.
//
// AN UNAVAILABLE PIPELINE IS A FAILURE, NOT AN ANSWER. The action logs and returns `unavailable`; this throws on it, so
// `withApiHandler` records it durably and the page shows its failed state rather than a thank-you nobody received.
// ---------------------------------------------------------------------------

const FIELDS = ['submissionId', 'requestType', 'propertyId', 'name', 'email', 'message', 'service', 'company'] as const

async function POSTHandler(req: NextRequest): Promise<Response> {
  let body: unknown
  try {
    body = await req.json()
  } catch {
    return NextResponse.json({ accepted: false, status: 'invalid' }, { status: 400 })
  }
  if (!body || typeof body !== 'object' || Array.isArray(body)) {
    return NextResponse.json({ accepted: false, status: 'invalid' }, { status: 400 })
  }

  const formData = new FormData()
  for (const field of FIELDS) {
    const value = (body as Record<string, unknown>)[field]
    if (typeof value === 'string' && value.length <= 5000) formData.set(field, value)
  }

  const result = await submitWebsiteIntake(formData)
  if (result.status === 'unavailable') {
    throw new Error(`Website intake unavailable for submission ${String(formData.get('submissionId') ?? '')}`)
  }
  return NextResponse.json(result, { status: result.accepted ? 200 : 422 })
}

export const POST = withApiHandler(
  { label: '/api/rust-ui/website-intake', route: '/api/rust-ui/website-intake' },
  POSTHandler,
)
