// ---------------------------------------------------------------------------
// ACCOUNTING V1 — controlled category lists (UI + light validation).
//
// Small, application-level controlled lists suitable for brokerage operations.
// No category-administration subsystem and no GL/chart table.
// ---------------------------------------------------------------------------

export const RECEIVABLE_CATEGORIES = [
  'COMMISSION',
  'LEASING_FEE',
  'MISC_INCOME',
  'OTHER',
] as const

export const EXPENSE_CATEGORIES = [
  'Marketing & Advertising',
  'Professional Fees',
  'Office',
  'Insurance',
  'MLS & Memberships',
  'Travel & Entertainment',
  'Merchant / Bank Fees',
  'Property / Deal Expense',
  'Other',
] as const

export type ExpenseCategory = (typeof EXPENSE_CATEGORIES)[number]

export function isExpenseCategory(value: string): value is ExpenseCategory {
  return (EXPENSE_CATEGORIES as readonly string[]).includes(value)
}
