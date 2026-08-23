"use client"

import Link from "next/link"
import { useMemo, useState } from "react"
import { PageHeader } from "@/components/portal/page-header"
import { Panel } from "@/components/portal/panel"
import type { Deal, DealStage } from "@/lib/portal/types"

const stageOrder: DealStage[] = [
  "new_lead",
  "qualified",
  "showing",
  "offer",
  "under_contract",
  "closed",
]

function formatCurrency(value?: number) {
  if (!value) return "—"

  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0,
  }).format(value)
}

function stageLabel(stage: DealStage) {
  switch (stage) {
    case "new_lead":
      return "New Lead"
    case "qualified":
      return "Qualified"
    case "showing":
      return "Showing"
    case "offer":
      return "Offer"
    case "under_contract":
      return "Under Contract"
    case "closed":
      return "Closed"
  }
}

function stageClasses(stage: DealStage) {
  switch (stage) {
    case "new_lead":
      return "bg-black/5 text-black/50"
    case "qualified":
      return "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]"
    case "showing":
      return "bg-[var(--portal-mist-2)] text-[var(--portal-navy-soft)]"
    case "offer":
      return "bg-[var(--portal-mist)] text-[var(--portal-navy)]"
    case "under_contract":
      return "bg-[var(--portal-success-pale)] text-[var(--portal-success)]"
    case "closed":
      return "bg-[var(--portal-navy)] text-white"
  }
}

export function DealsPortfolio({
  deals,
}: {
  deals: Deal[]
}) {
  const [stageFilter, setStageFilter] = useState<DealStage | "all">("all")

  const filteredDeals = useMemo(() => {
    if (stageFilter === "all") return deals
    return deals.filter((deal) => deal.stage === stageFilter)
  }, [deals, stageFilter])

  return (
    <div>
      <PageHeader compact eyebrow="Portfolio" title="Deals">
        <span className="text-xs font-light text-black/40">
          {filteredDeals.length} shown
        </span>
      </PageHeader>

      <div className="mb-4 flex flex-wrap gap-1.5">
        <button
          type="button"
          onClick={() => setStageFilter("all")}
          className={[
            "rounded-full border px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.12em] transition",
            stageFilter === "all"
              ? "border-[var(--portal-navy)] bg-[var(--portal-navy)] text-white"
              : "border-[var(--portal-panel-border)] bg-white/40 text-[var(--portal-navy-soft)] hover:border-[var(--portal-navy)]",
          ].join(" ")}
        >
          All
        </button>

        {stageOrder.map((stage) => (
          <button
            key={stage}
            type="button"
            onClick={() => setStageFilter(stage)}
            className={[
              "rounded-full border px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.12em] transition",
              stageFilter === stage
                ? "border-[var(--portal-navy)] bg-[var(--portal-navy)] text-white"
                : "border-[var(--portal-panel-border)] bg-white/40 text-[var(--portal-navy-soft)] hover:border-[var(--portal-navy)]",
            ].join(" ")}
          >
            {stageLabel(stage)}
          </button>
        ))}
      </div>

      <Panel compact variant="standard" divider flush>
        <div className="overflow-x-auto">
          <table className="min-w-[960px] w-full border-collapse">
            <thead>
              <tr className="border-b border-[var(--portal-border)] bg-[var(--portal-blue-pale)]/60">
                <TableHeading>Property</TableHeading>
                <TableHeading>Client</TableHeading>
                <TableHeading>Stage</TableHeading>
                <TableHeading>Price / Offer</TableHeading>
                <TableHeading>Next</TableHeading>
                <TableHeading>Owner</TableHeading>
                <TableHeading>{""}</TableHeading>
              </tr>
            </thead>
            <tbody>
              {filteredDeals.length > 0 ? (
                filteredDeals.map((deal) => (
                  <tr
                    key={deal.id}
                    className="border-b border-[var(--portal-border)] last:border-b-0 hover:bg-[var(--portal-blue-pale)]/45"
                  >
                    <td className="px-4 py-3 align-middle">
                      <div className="flex items-center gap-3">
                        {deal.heroMediaId ? (
                          <img
                            src={`/api/media/${deal.heroMediaId}`}
                            alt={deal.propertyName}
                            className="h-10 w-14 shrink-0 rounded-md object-cover"
                          />
                        ) : (
                          <div className="h-10 w-14 shrink-0 rounded-md bg-gradient-to-br from-[var(--portal-blue-pale)] to-[var(--portal-navy-soft)]" />
                        )}
                        <div className="min-w-0">
                          <Link
                            href={`/portal/deals/${deal.id}`}
                            className="block truncate text-sm font-medium text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]"
                          >
                            {deal.propertyName}
                          </Link>
                          <div className="truncate text-xs font-light text-black/45">
                            {deal.propertyLocation}
                          </div>
                        </div>
                      </div>
                    </td>
                    <td className="px-4 py-3 align-middle text-sm font-light">
                      {deal.clientName}
                    </td>
                    <td className="px-4 py-3 align-middle">
                      <span
                        className={[
                          "inline-flex rounded-full px-2.5 py-1 text-[10px] font-light uppercase tracking-[0.1em]",
                          stageClasses(deal.stage),
                        ].join(" ")}
                      >
                        {stageLabel(deal.stage)}
                      </span>
                    </td>
                    <td className="px-4 py-3 align-middle text-sm font-light tabular-nums">
                      {formatCurrency(
                        deal.latestOfferAmount ??
                          deal.offerPrice ??
                          deal.listPrice
                      )}
                    </td>
                    <td className="px-4 py-3 align-middle">
                      <div className="text-sm font-light">
                        {deal.nextMilestone ?? "—"}
                      </div>
                      <div className="text-xs font-light text-black/40">
                        {deal.nextMilestoneAt ?? ""}
                      </div>
                    </td>
                    <td className="px-4 py-3 align-middle text-sm font-light">
                      {deal.owner}
                    </td>
                    <td className="px-4 py-3 align-middle text-right">
                      <Link
                        href={`/portal/deals/${deal.id}`}
                        className="inline-flex min-h-8 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-2.5 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)]"
                      >
                        Open
                      </Link>
                    </td>
                  </tr>
                ))
              ) : (
                <tr>
                  <td
                    colSpan={7}
                    className="px-4 py-10 text-center text-sm font-light text-black/40"
                  >
                    No deals found.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </Panel>
    </div>
  )
}

function TableHeading({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <th className="px-4 py-3 text-left text-[10px] font-light uppercase tracking-[0.16em] text-[var(--portal-blue-gray)]">
      {children}
    </th>
  )
}
