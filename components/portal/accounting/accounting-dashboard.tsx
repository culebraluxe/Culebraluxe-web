'use client'

import Link from 'next/link'

import type { DashboardSummary, PnlLine } from '@/db/accounting'
import { formatDate, formatMoney } from '@/lib/accounting/format'
import {
  GlassPanel,
  MetricCard,
  PnlTrendChart,
} from '@/components/portal/accounting/accounting-shell'

const DONUT_COLORS = [
  '#c6a15b',
  '#7dd3fc',
  '#f0abfc',
  '#86efac',
  '#fcd34d',
  '#fca5a5',
  '#a5b4fc',
  '#93c5fd',
  '#e2e8f0',
]

// Shared desktop two-column grid so Row 2 (analytics) and Row 3 (money in/out)
// align on the exact same rails: identical tracks, gutter, and outer edges at
// desktop, while mobile/tablet still stack to a single column.
const TWO_COL_GRID = 'grid grid-cols-1 gap-4 lg:grid-cols-2'

export function AccountingDashboard({
  data,
  expenseCategories,
  expenseTotal,
}: {
  data: DashboardSummary
  /** Current-month expense category breakdown (reused P&L aggregation). */
  expenseCategories: PnlLine[]
  /** Current-month total = Σ POSTED expenses (reconciles to "Expenses This Month"). */
  expenseTotal: number
}) {
  // P&L summary over the same 6-month window as the trend graph (reused data).
  const trendIncome = data.pnlTrend.reduce((s, p) => s + p.income, 0)
  const trendExpenses = data.pnlTrend.reduce((s, p) => s + p.expenses, 0)
  const trendNet = trendIncome - trendExpenses

  // conic-gradient donut built from the reused category breakdown.
  let cursor = 0
  const gradientStops = expenseCategories
    .map((c, i) => {
      const frac = expenseTotal ? c.amount / expenseTotal : 0
      const from = cursor
      const to = cursor + frac * 360
      cursor = to
      return `${DONUT_COLORS[i % DONUT_COLORS.length]} ${from}deg ${to}deg`
    })
    .join(', ')

  return (
    <div className="space-y-4">
      {/* Summary cards */}
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
        <MetricCard
          label="Receivables"
          value={formatMoney(data.receivablesOutstanding)}
          hint="Outstanding (OPEN)"
        />
        <MetricCard
          label="Expenses This Month"
          value={formatMoney(data.expensesThisMonth)}
          hint="POSTED"
        />
        <MetricCard
          label="Net Income"
          value={formatMoney(data.netIncome)}
          hint="Realized income − posted expenses"
          tone={data.netIncome >= 0 ? 'good' : 'bad'}
        />
        <MetricCard
          label="Open / Overdue Receivables"
          value={`${data.openCount} / ${data.overdueCount}`}
          hint={`${data.overdueCount} past due`}
          tone={data.overdueCount > 0 ? 'warn' : 'neutral'}
        />
      </div>

      {/* ROW 2 — COMPACT ANALYTICS (shared two-column grid) */}
      <div className={TWO_COL_GRID}>
        <GlassPanel title="P&L / Net Income Trend">
          <div className="grid grid-cols-3 gap-3 border-b border-white/10 pb-3">
            <div>
              <p className="text-[9px] font-medium uppercase tracking-[0.16em] text-[var(--portal-gold)]">
                Total Income
              </p>
              <p className="mt-0.5 font-serif text-lg font-light text-white">
                {formatMoney(trendIncome)}
              </p>
            </div>
            <div>
              <p className="text-[9px] font-medium uppercase tracking-[0.16em] text-[var(--portal-gold)]">
                Total Expenses
              </p>
              <p className="mt-0.5 font-serif text-lg font-light text-sky-300">
                {formatMoney(trendExpenses)}
              </p>
            </div>
            <div>
              <p className="text-[9px] font-medium uppercase tracking-[0.16em] text-[var(--portal-gold)]">
                Net Income
              </p>
              <p
                className={`mt-0.5 font-serif text-lg font-light ${
                  trendNet >= 0 ? 'text-emerald-300' : 'text-rose-300'
                }`}
              >
                {formatMoney(trendNet)}
              </p>
            </div>
          </div>
          <div className="mt-2">
            <PnlTrendChart data={data.pnlTrend} />
          </div>
          <p className="mt-1 text-[9px] font-light text-white/40">
            Last 6 months · How are we doing?
          </p>
        </GlassPanel>

        <GlassPanel title="Expenses by Category · This Month">
          {expenseTotal > 0 ? (
            <div className="flex items-center gap-4">
              <div
                className="relative h-28 w-28 flex-none rounded-full"
                style={{ background: `conic-gradient(${gradientStops})` }}
              >
                <div className="absolute inset-[24%] flex flex-col items-center justify-center rounded-full bg-[var(--portal-navy-deep)]">
                  <span className="text-[9px] font-medium uppercase tracking-wide text-white/50">
                    Total
                  </span>
                  <span className="font-serif text-[11px] text-white/85">
                    {formatMoney(expenseTotal)}
                  </span>
                </div>
              </div>
              <ul className="min-w-0 flex-1 space-y-1">
                {expenseCategories.map((c, i) => {
                  const pct = expenseTotal ? (c.amount / expenseTotal) * 100 : 0
                  return (
                    <li key={c.label} className="flex items-center gap-2 text-[11px]">
                      <span
                        className="h-2 w-2 flex-none rounded-full"
                        style={{ background: DONUT_COLORS[i % DONUT_COLORS.length] }}
                      />
                      <span className="min-w-0 flex-1 truncate text-white/70">
                        {c.label}
                      </span>
                      <span className="flex-none text-[10px] text-white/40">
                        {pct.toFixed(0)}%
                      </span>
                      <span className="flex-none text-white/85">
                        {formatMoney(c.amount)}
                      </span>
                    </li>
                  )
                })}
              </ul>
            </div>
          ) : (
            <p className="py-6 text-sm font-light text-white/40">
              No posted expenses this month.
            </p>
          )}
        </GlassPanel>
      </div>

      {/* ROW 3 — MONEY IN / MONEY OUT (operational evidence) */}
      <div className={TWO_COL_GRID}>
        <GlassPanel
          title="Money In / Receivables"
          action={
            <Link
              href="/portal/accounting/receivables"
              className="text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-gold)] transition hover:text-white"
            >
              View Receivables
            </Link>
          }
        >
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs">
              <thead>
                <tr className="border-b border-white/10 text-[9px] font-medium uppercase tracking-[0.14em] text-white/40">
                  <th className="py-1.5 pr-3">Client / Property</th>
                  <th className="py-1.5 pr-3">Description</th>
                  <th className="py-1.5 pr-3 text-right">Amount</th>
                  <th className="py-1.5 pr-3">Due</th>
                  <th className="py-1.5">Status</th>
                </tr>
              </thead>
              <tbody>
                {data.recentActivity.slice(0, 6).map((r) => (
                  <tr key={r.id} className="border-b border-white/[0.05]">
                    <td className="py-2 pr-3 text-white/70">
                      {r.personName || r.propertyName || r.dealName || '—'}
                    </td>
                    <td className="py-2 pr-3 text-white/85">{r.description}</td>
                    <td className="py-2 pr-3 text-right text-white/90">
                      {formatMoney(r.amount)}
                    </td>
                    <td className="py-2 pr-3 text-white/60">
                      {formatDate(r.dueOn) || '—'}
                    </td>
                    <td className="py-2">
                      <span
                        className={`inline-flex rounded-full border px-1.5 py-0.5 text-[9px] font-medium uppercase tracking-wide ${
                          r.status === 'PAID'
                            ? 'border-emerald-400/40 text-emerald-300'
                            : r.status === 'VOID'
                              ? 'border-white/20 text-white/40'
                              : 'border-[var(--portal-gold)]/50 text-[var(--portal-gold)]'
                        }`}
                      >
                        {r.status}
                      </span>
                    </td>
                  </tr>
                ))}
                {data.recentActivity.length === 0 && (
                  <tr>
                    <td colSpan={5} className="py-4 text-center text-white/40">
                      No receivables yet.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </GlassPanel>

        <GlassPanel
          title="Money Out / Expenses"
          action={
            <Link
              href="/portal/accounting/expenses"
              className="text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-gold)] transition hover:text-white"
            >
              View Expenses
            </Link>
          }
        >
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs">
              <thead>
                <tr className="border-b border-white/10 text-[9px] font-medium uppercase tracking-[0.14em] text-white/40">
                  <th className="py-1.5 pr-3">Date</th>
                  <th className="py-1.5 pr-3">Vendor</th>
                  <th className="py-1.5 pr-3">Category</th>
                  <th className="py-1.5 text-right">Amount</th>
                </tr>
              </thead>
              <tbody>
                {data.recentExpenses.slice(0, 6).map((e) => (
                  <tr key={e.id} className="border-b border-white/[0.05]">
                    <td className="py-2 pr-3 text-white/60">{formatDate(e.expenseOn)}</td>
                    <td className="py-2 pr-3 text-white/85">{e.vendor}</td>
                    <td className="py-2 pr-3 text-white/70">{e.category}</td>
                    <td className="py-2 text-right text-sky-300">
                      {formatMoney(e.amount)}
                    </td>
                  </tr>
                ))}
                {data.recentExpenses.length === 0 && (
                  <tr>
                    <td colSpan={4} className="py-4 text-center text-white/40">
                      No expenses yet.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </GlassPanel>
      </div>
      {/* ROW 4 — RECEIPT / NEEDS REVIEW (compact horizontal action band) */}
      <GlassPanel title="Receipt / Needs Review">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <p className="text-sm font-light text-white/70">
            Capture a receipt and turn it into an expense.
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <Link
              href="/portal/accounting/receipt-scanner"
              className="inline-flex items-center justify-center rounded-md bg-[var(--portal-gold)] px-4 py-2 text-xs font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy-deep)] transition hover:brightness-110"
            >
              Scan New Receipt
            </Link>
            <Link
              href="/portal/accounting/receipt-scanner"
              className="rounded-md border border-white/20 px-3 py-2 text-[10px] font-medium uppercase tracking-[0.12em] text-white/70 transition hover:border-[var(--portal-gold)] hover:text-white"
            >
              Go to Receipt Scanner
            </Link>
          </div>
        </div>
        <p className="mt-2 text-[10px] font-light text-white/40">
          OCR is a later pass — the scanner is a V1 prototype. Later: receipts
          awaiting review · unknown bank classifications · unmatched vendors ·
          uncategorized expenses.
        </p>
      </GlassPanel>
    </div>
  )
}
