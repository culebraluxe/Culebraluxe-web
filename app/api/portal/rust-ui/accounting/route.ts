import { NextResponse, type NextRequest } from 'next/server'

import { getPortalActingUser } from '@/lib/auth/portal-session'
import { withApiHandler } from '@/lib/error-capture-seam'
import { accountingPayload, isAccountingScreen } from '@/lib/portal-rust-ui/accounting-payload'
import {
  rustApiCreateExpense,
  rustApiCreateReceivable,
  rustApiMarkReceivablePaid,
  RustApiError,
} from '@/lib/rust-api/client'

// ---------------------------------------------------------------------------
// THE ACCOUNTING COMMAND BRIDGE — thin, authenticated, and it answers with the refreshed screen.
//
// WHY ONE ROUTE PER DOMAIN RATHER THAN ONE PER COMMAND: the browser posts an `action` and the domain's arguments, this
// route translates that into the Rust service's own request shape, and then it returns the payload for the screen that
// asked — so a save and the refresh that follows it are ONE round trip and the screen can never show a stale list after
// writing a row.
//
// NO BUSINESS RULES LIVE HERE. Vendor-is-required, category-is-canonical, amount-is-a-decimal and the mark-paid
// transition are all Rust's; this file must not grow a second copy of them, because two copies of a rule is one copy too
// many and the browser would be the one enforcing the stale version.
// ---------------------------------------------------------------------------

type AccountingCommandBody = {
  action?: string
  screen?: string
  from?: string | null
  to?: string | null
  // CreateExpense / CreateReceivable
  vendor?: string
  category?: string
  amount?: string
  expenseOn?: string
  memo?: string | null
  reference?: string | null
  description?: string
  issuedOn?: string
  dueOn?: string | null
  dealId?: string | null
  propertyId?: string | null
  personId?: string | null
  // MarkReceivablePaid
  receivableId?: string
  paidOn?: string
}

function text(value: string | null | undefined): string | null {
  const trimmed = value?.trim()
  return trimmed ? trimmed : null
}

async function POSTHandler(req: NextRequest): Promise<Response> {
  const actor = await getPortalActingUser()
  if (!actor) {
    return NextResponse.json({ error: 'Unauthorized.' }, { status: 401 })
  }

  const body = (await req.json()) as AccountingCommandBody
  const screen = body.screen ?? ''
  if (!isAccountingScreen(screen)) {
    return NextResponse.json(
      { error: `no accounting screen '${screen}'` },
      { status: 400 },
    )
  }

  try {
    switch (body.action) {
      case 'createExpense': {
        await rustApiCreateExpense({
          vendor: body.vendor ?? '',
          category: body.category ?? '',
          // The amount travels as the digits the operator typed: a Number here would round it before Rust ever
          // validated it.
          amount: body.amount ?? '',
          expenseOn: body.expenseOn ?? '',
          memo: text(body.memo),
          dealId: text(body.dealId),
          propertyId: text(body.propertyId),
          personId: text(body.personId),
        })
        break
      }
      case 'createReceivable': {
        await rustApiCreateReceivable({
          reference: text(body.reference),
          description: body.description ?? '',
          category: body.category ?? '',
          amount: body.amount ?? '',
          issuedOn: body.issuedOn ?? '',
          dueOn: text(body.dueOn),
          dealId: text(body.dealId),
          propertyId: text(body.propertyId),
          personId: text(body.personId),
        })
        break
      }
      case 'markReceivablePaid': {
        const id = text(body.receivableId)
        if (!id) {
          return NextResponse.json({ error: 'A receivable is required.' }, { status: 400 })
        }
        await rustApiMarkReceivablePaid(id, { paidOn: body.paidOn ?? '' })
        break
      }
      default: {
        return NextResponse.json(
          { error: `no accounting command '${body.action ?? ''}'` },
          { status: 400 },
        )
      }
    }
  } catch (error) {
    // The service's own words, and its own status: a validation rule is a 400, a void or missing receivable is the 409
    // the operator needs to see rather than a generic failure.
    if (error instanceof RustApiError) {
      return NextResponse.json({ error: error.message }, { status: error.status })
    }
    throw error
  }

  // The write succeeded: answer with the screen as it now is.
  return NextResponse.json(await accountingPayload(screen, { from: body.from, to: body.to }))
}

export const POST = withApiHandler(
  { label: '/api/portal/rust-ui/accounting', route: '/api/portal/rust-ui/accounting' },
  POSTHandler,
)
