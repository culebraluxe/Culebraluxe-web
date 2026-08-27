'use client'

import { useActionState, useState } from 'react'

import type { Receivable } from '@/db/accounting'
import { RECEIVABLE_CATEGORIES } from '@/lib/accounting/categories'
import { formatDate, formatMoney, todayISO } from '@/lib/accounting/format'
import {
  createReceivableAction,
  markReceivablePaidAction,
  type AccountingWriteState,
} from '@/app/portal/accounting/actions'
import { GlassPanel } from '@/components/portal/accounting/accounting-shell'

const emptyCreate: AccountingWriteState = null

export function AccountingReceivables({ rows }: { rows: Receivable[] }) {
  const [showNew, setShowNew] = useState(false)
  const [createState, createAction, createPending] = useActionState(
    createReceivableAction,
    emptyCreate,
  )
  const [paidState, paidAction, paidPending] = useActionState(
    markReceivablePaidAction,
    emptyCreate,
  )

  const today = todayISO()

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-sm font-light text-white/60">
          {rows.length} receivable{rows.length === 1 ? '' : 's'}
        </p>
        <button
          type="button"
          onClick={() => setShowNew((v) => !v)}
          className="rounded-[var(--portal-panel-radius)] border border-[var(--portal-gold)]/60 bg-[var(--portal-gold)]/15 px-4 py-2 text-xs font-medium uppercase tracking-[0.14em] text-[var(--portal-gold)] transition hover:bg-[var(--portal-gold)]/25"
        >
          {showNew ? 'Cancel' : '+ New Receivable'}
        </button>
      </div>

      {showNew && (
        <GlassPanel title="New Receivable">
          <form
            action={createAction}
            className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3"
          >
            <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60">
              Reference
              <input
                name="reference"
                placeholder="e.g. 2026-014 · Commission"
                className="mt-1 w-full rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white placeholder:text-white/30 focus:border-[var(--portal-gold)] focus:outline-none"
              />
            </label>
            <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60">
              Description *
              <input
                name="description"
                required
                className="mt-1 w-full rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white placeholder:text-white/30 focus:border-[var(--portal-gold)] focus:outline-none"
              />
            </label>
            <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60">
              Category
              <select
                name="category"
                defaultValue="COMMISSION"
                className="mt-1 w-full rounded-md border border-white/15 bg-[#0b1220] px-3 py-2 text-sm text-white focus:border-[var(--portal-gold)] focus:outline-none"
              >
                {RECEIVABLE_CATEGORIES.map((c) => (
                  <option key={c} value={c}>
                    {c.replaceAll('_', ' ')}
                  </option>
                ))}
              </select>
            </label>
            <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60">
              Amount (USD) *
              <input
                name="amount"
                type="number"
                min="0"
                step="0.01"
                required
                className="mt-1 w-full rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white placeholder:text-white/30 focus:border-[var(--portal-gold)] focus:outline-none"
              />
            </label>
            <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60">
              Issue Date
              <input
                name="issuedOn"
                type="date"
                defaultValue={today}
                className="mt-1 w-full rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white focus:border-[var(--portal-gold)] focus:outline-none"
              />
            </label>
            <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60">
              Due Date
              <input
                name="dueOn"
                type="date"
                className="mt-1 w-full rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white focus:border-[var(--portal-gold)] focus:outline-none"
              />
            </label>
            <div className="flex items-end gap-3 sm:col-span-2 lg:col-span-3">
              <button
                type="submit"
                disabled={createPending}
                className="rounded-md bg-[var(--portal-gold)] px-4 py-2 text-xs font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy-deep)] transition hover:brightness-110 disabled:opacity-50"
              >
                {createPending ? 'Creating…' : 'Create Receivable'}
              </button>
              {createState?.error && (
                <p className="text-xs text-rose-300">{createState.error}</p>
              )}
              {createState?.ok && (
                <p className="text-xs text-emerald-300">Created.</p>
              )}
            </div>
          </form>
        </GlassPanel>
      )}

      <GlassPanel title="All Receivables">
        <div className="overflow-x-auto">
          <table className="w-full min-w-[760px] text-left text-sm">
            <thead>
              <tr className="border-b border-white/10 text-[10px] font-medium uppercase tracking-[0.16em] text-white/45">
                <th className="py-2 pr-3">Reference</th>
                <th className="py-2 pr-3">Client / Property</th>
                <th className="py-2 pr-3">Description / Category</th>
                <th className="py-2 pr-3 text-right">Amount</th>
                <th className="py-2 pr-3">Issue</th>
                <th className="py-2 pr-3">Due</th>
                <th className="py-2 pr-3">Status</th>
                <th className="py-2 text-right">Action</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => {
                const overdue =
                  r.status === 'OPEN' && !!r.dueOn && r.dueOn < today
                return (
                  <tr
                    key={r.id}
                    className="border-b border-white/[0.06] hover:bg-white/[0.03]"
                  >
                    <td className="py-2.5 pr-3 text-white/85">
                      {r.reference || '—'}
                    </td>
                    <td className="py-2.5 pr-3 text-white/70">
                      {r.personName || r.propertyName || r.dealName || '—'}
                    </td>
                    <td className="py-2.5 pr-3">
                      <p className="text-white/85">{r.description}</p>
                      <p className="text-[11px] text-white/45">
                        {r.category.replaceAll('_', ' ')}
                      </p>
                    </td>
                    <td className="py-2.5 pr-3 text-right font-medium text-white">
                      {formatMoney(r.amount)}
                    </td>
                    <td className="py-2.5 pr-3 text-white/60">
                      {formatDate(r.issuedOn)}
                    </td>
                    <td className="py-2.5 pr-3 text-white/60">
                      {formatDate(r.dueOn) || '—'}
                    </td>
                    <td className="py-2.5 pr-3">
                      <span
                        className={`inline-flex rounded-full border px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide ${
                          r.status === 'PAID'
                            ? 'border-emerald-400/40 text-emerald-300'
                            : r.status === 'VOID'
                              ? 'border-white/20 text-white/40'
                              : overdue
                                ? 'border-amber-400/50 text-amber-300'
                                : 'border-[var(--portal-gold)]/50 text-[var(--portal-gold)]'
                        }`}
                      >
                        {overdue ? 'Overdue' : r.status}
                      </span>
                    </td>
                    <td className="py-2.5 text-right">
                      {r.status === 'OPEN' && (
                        <form
                          action={paidAction}
                          className="flex items-center justify-end gap-2"
                        >
                          <input type="hidden" name="id" value={r.id} />
                          <input
                            name="paidOn"
                            type="date"
                            defaultValue={today}
                            className="w-[7.5rem] rounded border border-white/15 bg-white/5 px-2 py-1 text-xs text-white focus:border-[var(--portal-gold)] focus:outline-none"
                          />
                          <button
                            type="submit"
                            disabled={paidPending}
                            className="rounded border border-emerald-400/50 px-2 py-1 text-[10px] font-medium uppercase tracking-wide text-emerald-300 transition hover:bg-emerald-400/15 disabled:opacity-50"
                          >
                            Mark Paid
                          </button>
                        </form>
                      )}
                    </td>
                  </tr>
                )
              })}
              {rows.length === 0 && (
                <tr>
                  <td
                    colSpan={8}
                    className="py-6 text-center text-sm font-light text-white/40"
                  >
                    No receivables yet.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
        {paidState?.error && (
          <p className="mt-2 text-xs text-rose-300">{paidState.error}</p>
        )}
      </GlassPanel>
    </div>
  )
}

