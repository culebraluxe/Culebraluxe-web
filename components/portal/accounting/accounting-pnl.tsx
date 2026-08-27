'use client'

import type { PnlStatement } from '@/db/accounting'
import { formatMoney } from '@/lib/accounting/format'
import { GlassPanel } from '@/components/portal/accounting/accounting-shell'

export function AccountingPnl({
  statement,
}: {
  statement: PnlStatement
}) {
  const rows = [
    ...statement.income.map((l) => ({
      label: l.label.replaceAll('_', ' '),
      amount: l.amount,
      kind: 'income' as const,
    })),
    ...statement.expenses.map((l) => ({
      label: l.label,
      amount: l.amount,
      kind: 'expense' as const,
    })),
  ]

  return (
    <div className="space-y-4">
      {/* Date range (server-rendered re-derive) */}
      <GlassPanel title="Date Range">
        <form
          action="/portal/accounting/pnl"
          method="get"
          className="flex flex-wrap items-end gap-3"
        >
          <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60">
            From
            <input
              name="from"
              type="date"
              defaultValue={statement.from}
              className="mt-1 block w-44 rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white focus:border-[var(--portal-gold)] focus:outline-none"
            />
          </label>
          <label className="block text-[11px] font-medium uppercase tracking-wide text-white/60">
            To
            <input
              name="to"
              type="date"
              defaultValue={statement.to}
              className="mt-1 block w-44 rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white focus:border-[var(--portal-gold)] focus:outline-none"
            />
          </label>
          <button
            type="submit"
            className="rounded-md border border-[var(--portal-gold)]/60 bg-[var(--portal-gold)]/15 px-4 py-2 text-xs font-medium uppercase tracking-[0.14em] text-[var(--portal-gold)] transition hover:bg-[var(--portal-gold)]/25"
          >
            Apply
          </button>
        </form>
      </GlassPanel>

      {/* Compact statement */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <GlassPanel title="Income">
          <ul className="divide-y divide-white/10">
            {statement.income.map((l) => (
              <li
                key={l.label}
                className="flex items-center justify-between py-2.5 text-sm"
              >
                <span className="text-white/75">{l.label.replaceAll('_', ' ')}</span>
                <span className="font-medium text-white">
                  {formatMoney(l.amount)}
                </span>
              </li>
            ))}
            {statement.income.length === 0 && (
              <li className="py-2 text-sm font-light text-white/40">
                No income in this range.
              </li>
            )}
          </ul>
          <div className="mt-2 flex items-center justify-between border-t border-white/15 pt-3 text-sm">
            <span className="font-medium uppercase tracking-wide text-white/70">
              Total Income
            </span>
            <span className="font-serif text-lg text-white">
              {formatMoney(statement.totalIncome)}
            </span>
          </div>
        </GlassPanel>

        <GlassPanel title="Expenses">
          <ul className="divide-y divide-white/10">
            {statement.expenses.map((l) => (
              <li
                key={l.label}
                className="flex items-center justify-between py-2.5 text-sm"
              >
                <span className="text-white/75">{l.label}</span>
                <span className="font-medium text-sky-300">
                  {formatMoney(l.amount)}
                </span>
              </li>
            ))}
            {statement.expenses.length === 0 && (
              <li className="py-2 text-sm font-light text-white/40">
                No expenses in this range.
              </li>
            )}
          </ul>
          <div className="mt-2 flex items-center justify-between border-t border-white/15 pt-3 text-sm">
            <span className="font-medium uppercase tracking-wide text-white/70">
              Total Expenses
            </span>
            <span className="font-serif text-lg text-white">
              {formatMoney(statement.totalExpenses)}
            </span>
          </div>
        </GlassPanel>
      </div>

      {/* Net income band */}
      <div
        className={`rounded-[var(--portal-panel-radius)] border p-5 ${
          statement.netIncome >= 0
            ? 'border-emerald-400/30 bg-emerald-400/10'
            : 'border-rose-400/30 bg-rose-400/10'
        }`}
      >
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="text-[10px] font-medium uppercase tracking-[0.2em] text-white/60">
              Net Income
            </p>
            <p className="mt-1 text-xs font-light text-white/50">
              Realized income − posted expenses · {rows.length} line items
            </p>
          </div>
          <p
            className={`font-serif text-3xl font-light ${
              statement.netIncome >= 0 ? 'text-emerald-300' : 'text-rose-300'
            }`}
          >
            {formatMoney(statement.netIncome)}
          </p>
        </div>
      </div>
    </div>
  )
}
