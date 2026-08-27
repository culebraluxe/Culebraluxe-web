// ---------------------------------------------------------------------------
// ACCOUNTING V1 — DEV-only deterministic fixture seed (pitch data).
//
// Seeds a small, believable set of receivables + expenses into the canonical
// DEV tables (account_receivable / account_expense) so the Accounting module
// renders real values for visual QA. References existing DEV Deal/Property/
// Person IDs. NEVER runs outside development. Idempotent: skips if the tables
// already contain rows.
//
// Run:  APP_ENV=development node --env-file=.env.local --import tsx scripts/seed-accounting-fixture.ts
// ---------------------------------------------------------------------------

import { sql } from '../db/client'
import {
  createExpense,
  createReceivable,
  markReceivablePaid,
} from '../db/accounting'

const env = process.env.APP_ENV ?? process.env.NODE_ENV
if (env !== 'development') {
  console.error('[seed-accounting-fixture] REFUSED: DEV-only. APP_ENV=', env)
  process.exit(1)
}

// Real canonical DEV references (stable UUIDs, not slugs).
const DEAL_CASA_HORIZONTE = 'eb7cbd90-2a9f-46a3-bb86-969f9dd09e65'
const PROP_CASA_HORIZONTE = '40000000-0000-4000-8000-000000000006'
const PROP_BRISAS_DEL_MAR = '40000000-0000-4000-8000-000000000004'
const PROP_CASA_BRISA = '40000000-0000-4000-8000-000000000005'
const PERSON_LEAD = 'c97333df-703c-4e8a-ad82-1cdf457dd443'

function iso(daysOffset: number): string {
  const d = new Date()
  d.setDate(d.getDate() + daysOffset)
  return d.toISOString().slice(0, 10)
}

async function main() {
  const [recCount, expCount] = await Promise.all([
    sql`select count(*)::int as c from account_receivable`,
    sql`select count(*)::int as c from account_expense`,
  ])
  if ((recCount[0] as { c?: number }).c! > 0 || (expCount[0] as { c?: number }).c! > 0) {
    console.log('[seed-accounting-fixture] non-empty — skipping (idempotent).')
    return
  }

  const receivables = [
    {
      reference: 'FIX-2026-001',
      description: 'Commission — Casa Horizonte sale',
      category: 'COMMISSION',
      amount: 37500,
      issuedOn: iso(-20),
      dueOn: iso(-10), // overdue
      dealId: DEAL_CASA_HORIZONTE,
      paidOn: null,
    },
    {
      reference: 'FIX-2026-002',
      description: 'Leasing fee — Brisas del Mar',
      category: 'LEASING_FEE',
      amount: 4250,
      issuedOn: iso(-40),
      dueOn: iso(-12),
      propertyId: PROP_BRISAS_DEL_MAR,
      paidOn: iso(-30), // paid last month
    },
    {
      reference: 'FIX-2026-003',
      description: 'Commission — 14 Elm Place',
      category: 'COMMISSION',
      amount: 18750,
      issuedOn: iso(-3),
      dueOn: iso(27),
      propertyId: PROP_CASA_BRISA,
      paidOn: null,
    },
    {
      reference: 'FIX-2026-004',
      description: 'Marketing reimbursement — listing',
      category: 'MISC_INCOME',
      amount: 600,
      issuedOn: iso(-5),
      dueOn: iso(25),
      propertyId: PROP_CASA_HORIZONTE,
      paidOn: null,
    },
    {
      reference: 'FIX-2026-005',
      description: 'Commission — Casa Brisa',
      category: 'COMMISSION',
      amount: 22400,
      issuedOn: iso(-15),
      dueOn: iso(15),
      propertyId: PROP_CASA_BRISA,
      paidOn: iso(-2), // paid this month
    },
    {
      reference: 'FIX-2026-006',
      description: 'Consulting fee',
      category: 'OTHER',
      amount: 1500,
      issuedOn: iso(-60),
      dueOn: iso(-55), // overdue
      personId: PERSON_LEAD,
      paidOn: null,
    },
  ]

  for (const r of receivables) {
    const { id } = await createReceivable({
      reference: r.reference,
      description: r.description,
      category: r.category,
      amount: r.amount,
      issuedOn: r.issuedOn,
      dueOn: r.dueOn,
      dealId: r.dealId,
      propertyId: r.propertyId,
      personId: r.personId,
    })
    if (r.paidOn) await markReceivablePaid(id, r.paidOn)
    console.log(`[seed] receivable ${r.reference} (${r.category}, ${r.amount})`)
  }

  const expenses = [
    { vendor: 'Culebra MLS', category: 'MLS & Memberships', amount: 895, expenseOn: iso(-2) },
    { vendor: 'Luxe Print Studio', category: 'Marketing & Advertising', amount: 420, expenseOn: iso(-5) },
    { vendor: 'Culebra Office Co', category: 'Office', amount: 1100, expenseOn: iso(-8) },
    { vendor: 'Tropical Insurance', category: 'Insurance', amount: 1650, expenseOn: iso(-12) },
    { vendor: 'De Leon Legal', category: 'Professional Fees', amount: 2400, expenseOn: iso(-15) },
    { vendor: 'Metro Maintenance', category: 'Property / Deal Expense', amount: 512.5, expenseOn: iso(-20), propertyId: PROP_CASA_HORIZONTE },
    { vendor: 'Card Processing', category: 'Merchant / Bank Fees', amount: 98.4, expenseOn: iso(-3) },
    { vendor: 'Isla Dining', category: 'Travel & Entertainment', amount: 187.25, expenseOn: iso(-10) },
  ]

  for (const e of expenses) {
    await createExpense({
      vendor: e.vendor,
      category: e.category,
      amount: e.amount,
      expenseOn: e.expenseOn,
      memo: 'DEV fixture',
      propertyId: (e as { propertyId?: string }).propertyId,
    })
    console.log(`[seed] expense ${e.vendor} (${e.category}, ${e.amount})`)
  }

  console.log('[seed-accounting-fixture] done.')
}
void main()
