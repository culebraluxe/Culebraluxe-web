'use client'

import { useActionState, useState } from 'react'

import { EXPENSE_CATEGORIES } from '@/lib/accounting/categories'
import { formatMoney } from '@/lib/accounting/format'
import { createExpenseAction, type AccountingWriteState } from '@/app/portal/accounting/actions'
import { GlassPanel } from '@/components/portal/accounting/accounting-shell'

const emptyCreate: AccountingWriteState = null

// Deterministic demo receipts. NOT real OCR — a fixed prototype seed so Lisa can
// see the intended future workflow. The "AI/OCR" claim is explicitly NOT made.
const DEMO_RECEIPTS = [
  {
    vendor: 'Metro Maintenance Co.',
    amount: 412.5,
    category: 'Property / Deal Expense',
    memo: 'Walkthrough cleanup — demo receipt',
  },
  {
    vendor: 'Wells Fargo Merchant Services',
    amount: 89.0,
    category: 'Merchant / Bank Fees',
    memo: 'Monthly processing — demo receipt',
  },
  {
    vendor: 'State Insurance Group',
    amount: 1200.0,
    category: 'Insurance',
    memo: 'E&O premium — demo receipt',
  },
  {
    vendor: 'Luxe Signage & Print',
    amount: 235.75,
    category: 'Marketing & Advertising',
    memo: 'Listing collateral — demo receipt',
  },
]

const WORKFLOW_STEPS = [
  { label: 'Receipt image', note: 'Scan or upload' },
  { label: 'AI/OCR extracts', note: 'Vendor · Date · Amount · Category' },
  { label: 'Lisa reviews', note: 'Confirm + add context' },
  { label: 'Create expense', note: 'Saved to Expenses' },
]

export function AccountingReceiptScanner() {
  const [fileName, setFileName] = useState<string | null>(null)
  const [dragging, setDragging] = useState(false)
  const [demoIndex, setDemoIndex] = useState(0)
  const [draft, setDraft] = useState<(typeof DEMO_RECEIPTS)[number] | null>(null)
  const [expenseOn] = useState(() => new Date().toISOString().slice(0, 10))
  const [createState, createAction, createPending] = useActionState(
    createExpenseAction,
    emptyCreate,
  )

  const scan = () => {
    // Deterministic fake extraction — cycles the seed, no real OCR.
    const next = DEMO_RECEIPTS[demoIndex % DEMO_RECEIPTS.length]
    setDraft(next)
    setDemoIndex((i) => i + 1)
    if (!fileName) setFileName('demo-receipt.jpg')
  }

  return (
    <div className="space-y-4">
      {/* Workflow strip */}
      <div className="flex flex-wrap items-center gap-2">
        {WORKFLOW_STEPS.map((step, i) => (
          <div key={step.label} className="flex items-center gap-2">
            <div className="rounded-[var(--portal-panel-radius)] border border-white/10 bg-white/[0.05] px-3 py-2">
              <p className="text-[10px] font-medium uppercase tracking-[0.16em] text-[var(--portal-gold)]">
                Step {i + 1}
              </p>
              <p className="text-xs text-white/80">{step.label}</p>
              <p className="text-[10px] text-white/40">{step.note}</p>
            </div>
            {i < WORKFLOW_STEPS.length - 1 && (
              <span className="text-white/25">→</span>
            )}
          </div>
        ))}
      </div>

      {/* Drop surface */}
      <GlassPanel title="Scan / Upload Receipt">
        <div
          onDragOver={(e) => {
            e.preventDefault()
            setDragging(true)
          }}
          onDragLeave={() => setDragging(false)}
          onDrop={(e) => {
            e.preventDefault()
            setDragging(false)
            const file = e.dataTransfer.files?.[0]
            if (file) setFileName(file.name)
          }}
          className={`flex flex-col items-center justify-center rounded-[var(--portal-panel-radius)] border-2 border-dashed px-6 py-12 text-center transition ${
            dragging
              ? 'border-[var(--portal-gold)] bg-[var(--portal-gold)]/10'
              : 'border-white/15 bg-white/[0.02]'
          }`}
        >
          <div className="text-3xl">🖼️</div>
          <p className="mt-3 text-sm text-white/80">
            {fileName ? fileName : 'Drag & drop a receipt image here'}
          </p>
          <p className="mt-1 max-w-md text-[11px] font-light text-white/45">
            Drop an image to attach it to this demo. For this prototype pass the
            surface is mostly visual — real OCR arrives in a later story.
          </p>
          <button
            type="button"
            onClick={scan}
            className="mt-4 rounded-md bg-[var(--portal-gold)] px-5 py-2.5 text-xs font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy-deep)] transition hover:brightness-110"
          >
            Scan Receipt
          </button>
        </div>

        <p className="mt-3 flex items-center gap-1.5 text-[11px] text-amber-300/90">
          <span>•</span>
          Prototype preview — extraction is a deterministic demo seed, not OCR.
        </p>
      </GlassPanel>

      {/* Draft expense preview */}
      {draft && (
        <GlassPanel title="Extracted Draft — Review Before Saving">
          <form action={createAction} className="space-y-3">
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
              <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60">
                Vendor
                <input
                  name="vendor"
                  readOnly
                  value={draft.vendor}
                  className="mt-1 w-full rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white"
                />
              </label>
              <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60">
                Date
                <input
                  name="expenseOn"
                  type="date"
                  value={expenseOn}
                  onChange={() => {}}
                  className="mt-1 w-full rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white"
                />
              </label>
              <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60">
                Amount
                <input
                  name="amount"
                  readOnly
                  value={draft.amount}
                  className="mt-1 w-full rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white"
                />
              </label>
              <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60">
                Category
                <select
                  name="category"
                  value={draft.category}
                  onChange={() => {}}
                  className="mt-1 w-full rounded-md border border-white/15 bg-[#0b1220] px-3 py-2 text-sm text-white"
                >
                  {EXPENSE_CATEGORIES.map((c) => (
                    <option key={c} value={c}>
                      {c}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <div className="flex flex-wrap items-center gap-3">
              <button
                type="submit"
                disabled={createPending}
                className="rounded-md bg-[var(--portal-gold)] px-4 py-2 text-xs font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy-deep)] transition hover:brightness-110 disabled:opacity-50"
              >
                {createPending ? 'Creating…' : 'Create Expense'}
              </button>
              <span className="text-xs font-medium text-white/70">
                {draft.memo} · {formatMoney(draft.amount)}
              </span>
              {createState?.error && (
                <span className="text-xs text-rose-300">{createState.error}</span>
              )}
              {createState?.ok && (
                <span className="text-xs text-emerald-300">
                  Saved to Expenses.
                </span>
              )}
            </div>
          </form>
        </GlassPanel>
      )}
    </div>
  )
}

