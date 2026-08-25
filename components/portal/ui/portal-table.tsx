import type { HTMLAttributes, ReactNode, TableHTMLAttributes, TdHTMLAttributes, ThHTMLAttributes } from "react"
import { ChevronLeft, ChevronRight } from "lucide-react"

import { cn } from "@/lib/utils"

export function PortalTable({
  className,
  children,
  ...props
}: TableHTMLAttributes<HTMLTableElement>) {
  return (
    <div className="overflow-x-auto [scrollbar-width:thin]">
      <table
        className={cn("w-full min-w-[760px] border-collapse text-left text-sm", className)}
        {...props}
      >
        {children}
      </table>
    </div>
  )
}

export function PortalTableHead({
  className,
  ...props
}: HTMLAttributes<HTMLTableSectionElement>) {
  return <thead className={cn("bg-[var(--portal-blue-pale)]", className)} {...props} />
}

export function PortalTableBody({
  className,
  ...props
}: HTMLAttributes<HTMLTableSectionElement>) {
  return <tbody className={className} {...props} />
}

export function PortalTableRow({
  className,
  ...props
}: HTMLAttributes<HTMLTableRowElement>) {
  return (
    <tr
      className={cn(
        "border-b border-[var(--portal-border)] transition-colors last:border-b-0 hover:bg-[var(--portal-blue-pale)]/50",
        className,
      )}
      {...props}
    />
  )
}

export function PortalTableHeader({
  className,
  ...props
}: ThHTMLAttributes<HTMLTableCellElement>) {
  return (
    <th
      className={cn(
        "px-4 py-3 text-[10px] font-light uppercase tracking-[0.18em] text-black/45 sm:px-6",
        className,
      )}
      {...props}
    />
  )
}

export function PortalTableCell({
  className,
  ...props
}: TdHTMLAttributes<HTMLTableCellElement>) {
  return <td className={cn("px-4 py-3.5 font-light text-black/60 sm:px-6", className)} {...props} />
}

export function PortalPagination({
  onNext,
  onPrevious,
  page,
  pageCount,
  totalLabel,
}: {
  onNext?: () => void
  onPrevious?: () => void
  page: number
  pageCount: number
  totalLabel?: ReactNode
}) {
  const buttonClass =
    "inline-flex min-h-11 items-center justify-center gap-1.5 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-3 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-gold)]/60 disabled:cursor-not-allowed disabled:opacity-35"

  return (
    <nav
      aria-label="Table pagination"
      className="flex flex-wrap items-center justify-between gap-3 border-t border-[var(--portal-panel-border)] px-4 py-3 sm:px-6"
    >
      <p className="text-xs font-light text-black/45">
        Page {page} of {pageCount}
        {totalLabel ? <> · {totalLabel}</> : null}
      </p>
      <div className="flex items-center gap-2">
        <button type="button" className={buttonClass} onClick={onPrevious} disabled={page <= 1}>
          <ChevronLeft className="h-3.5 w-3.5" /> Previous
        </button>
        <button type="button" className={buttonClass} onClick={onNext} disabled={page >= pageCount}>
          Next <ChevronRight className="h-3.5 w-3.5" />
        </button>
      </div>
    </nav>
  )
}
