'use server'

import { revalidatePath } from 'next/cache'
import { redirect } from 'next/navigation'

import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'
import {
  createExpense,
  createReceivable,
  markReceivablePaid,
} from '@/db/accounting'

// ACCOUNTING V1 — server actions (UI command layer). Reuse the canonical
// accounting write seams; the React client never touches SQL. All actions are
// portal.read-gated and revalidate the accounting routes so the server-projected
// rows stay canonical.

export type AccountingWriteState = {
  ok: boolean
  error?: string
  id?: string
} | null

async function requireRead(): Promise<void> {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    'portal.read',
  )
  if (!access.ok) redirect(access.redirectTo)
}

function parseAmount(raw: string): number | null {
  const value = Number(raw)
  return Number.isFinite(value) ? value : null
}

export async function createReceivableAction(
  _prev: AccountingWriteState,
  formData: FormData,
): Promise<AccountingWriteState> {
  await requireRead()

  const description = String(formData.get('description') ?? '').trim()
  const amount = parseAmount(String(formData.get('amount') ?? ''))
  if (!description) return { ok: false, error: 'Description is required.' }
  if (amount === null || amount < 0)
    return { ok: false, error: 'Amount must be a non-negative number.' }

  try {
    const { id } = await createReceivable({
      reference: String(formData.get('reference') ?? '').trim() || null,
      description,
      category: String(formData.get('category') ?? 'COMMISSION').trim() || 'COMMISSION',
      amount,
      issuedOn: String(formData.get('issuedOn') ?? new Date().toISOString().slice(0, 10)),
      dueOn: String(formData.get('dueOn') ?? '').trim() || null,
    })
    revalidatePath('/portal/accounting')
    revalidatePath('/portal/accounting/receivables')
    return { ok: true, id }
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : 'Could not create the receivable.',
    }
  }
}

export async function markReceivablePaidAction(
  _prev: AccountingWriteState,
  formData: FormData,
): Promise<AccountingWriteState> {
  await requireRead()

  const id = String(formData.get('id') ?? '').trim()
  const paidOn = String(formData.get('paidOn') ?? new Date().toISOString().slice(0, 10)).trim()
  if (!id) return { ok: false, error: 'Missing receivable.' }

  try {
    await markReceivablePaid(id, paidOn)
    revalidatePath('/portal/accounting')
    revalidatePath('/portal/accounting/receivables')
    return { ok: true, id }
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : 'Could not mark the receivable paid.',
    }
  }
}

export async function createExpenseAction(
  _prev: AccountingWriteState,
  formData: FormData,
): Promise<AccountingWriteState> {
  await requireRead()

  const vendor = String(formData.get('vendor') ?? '').trim()
  const amount = parseAmount(String(formData.get('amount') ?? ''))
  if (!vendor) return { ok: false, error: 'Vendor is required.' }
  if (amount === null || amount < 0)
    return { ok: false, error: 'Amount must be a non-negative number.' }

  try {
    const { id } = await createExpense({
      vendor,
      category: String(formData.get('category') ?? '').trim(),
      amount,
      expenseOn: String(formData.get('expenseOn') ?? new Date().toISOString().slice(0, 10)),
      memo: String(formData.get('memo') ?? '').trim() || null,
    })
    revalidatePath('/portal/accounting')
    revalidatePath('/portal/accounting/expenses')
    return { ok: true, id }
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : 'Could not create the expense.',
    }
  }
}
