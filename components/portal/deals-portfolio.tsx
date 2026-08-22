"use client"

import Link from "next/link"
import { useMemo, useState } from "react"
import { PageHeader } from "@/components/portal/page-header"
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

function dealSummary(deal: Deal): string {
  const parts: string[] = []
  if (deal.showingCount) {
    parts.push(`${deal.showingCount} showing${deal.showingCount === 1 ? "" : "s"}`)
  }
  if (deal.offerCount) {
    parts.push(`${deal.offerCount} offer${deal.offerCount === 1 ? "" : "s"}`)
  }
  if (deal.participantCount) {
    parts.push(`${deal.participantCount} participant${deal.participantCount === 1 ? "" : "s"}`)
  }
  return parts.join(" · ")
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

  const activeDeals = deals.filter((deal) => deal.stage !== "closed")

  const underContract = deals.filter(
    (deal) => deal.stage === "under_contract"
  )

  const averageListingPrice =
    deals.length > 0
      ? deals.reduce((sum, deal) => sum + (deal.listPrice ?? 0), 0) /
        deals.length
      : 0

  const closingSoon = deals.filter((deal) => deal.closingDate)

  return (
    <div>
      <PageHeader
        eyebrow="Portfolio"
        title="Deals Portfolio"
        subtitle="A working view of active opportunities, properties, and transaction progress."
      />

      <div className="mb-6 flex flex-wrap gap-2">
        <button
          type="button"
          onClick={() => setStageFilter("all")}
          className={[
            "rounded-full border px-4 py-2 text-xs font-light uppercase tracking-[0.12em] transition",
            stageFilter === "all"
              ? "border-[var(--portal-navy)] bg-[var(--portal-navy)] text-white"
              : "border-[var(--portal-border)] bg-white text-[var(--portal-navy-soft)] hover:border-[var(--portal-navy)]",
          ].join(" ")}
        >
          All Deals
        </button>

        {stageOrder.map((stage) => (
          <button
            key={stage}
            type="button"
            onClick={() => setStageFilter(stage)}
            className={[
              "rounded-full border px-4 py-2 text-xs font-light uppercase tracking-[0.12em] transition",
              stageFilter === stage
                ? "border-[var(--portal-navy)] bg-[var(--portal-navy)] text-white"
                : "border-[var(--portal-border)] bg-white text-[var(--portal-navy-soft)] hover:border-[var(--portal-navy)]",
            ].join(" ")}
          >
            {stageLabel(stage)}
          </button>
        ))}
      </div>

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          label="Active Deals"
          value={String(activeDeals.length)}
          detail="Open opportunities"
        />

        <MetricCard
          label="Under Contract"
          value={String(underContract.length)}
          detail="Transactions in progress"
        />

        <MetricCard
          label="Avg Listing Price"
          value={formatCurrency(averageListingPrice)}
          detail="Across current portfolio"
        />

        <MetricCard
          label="Closing Pipeline"
          value={String(closingSoon.length)}
          detail="Deals with closing dates"
        />
      </section>

      <section className="mt-6 overflow-hidden rounded-sm border border-[var(--portal-border)] bg-white">
        <div className="border-b border-[var(--portal-border)] px-6 py-5">
          <div className="flex items-end justify-between gap-4">
            <div>
              <h2 className="font-serif text-2xl font-light">
                Active Portfolio
              </h2>

              <p className="mt-1 text-xs font-light text-black/40">
                Client, property, stage, value and next milestone.
              </p>
            </div>

            <div className="text-xs font-light text-black/35">
              {filteredDeals.length} deals
            </div>
          </div>
        </div>

        <div className="overflow-x-auto">
          <table className="min-w-[1100px] w-full border-collapse">
            <thead>
              <tr className="border-b border-[var(--portal-border)] bg-[var(--portal-blue-pale)]">
                <TableHeading>Property</TableHeading>
                <TableHeading>Client</TableHeading>
                <TableHeading>Stage</TableHeading>
                <TableHeading>Price / Offer</TableHeading>
                <TableHeading>Next Milestone</TableHeading>
                <TableHeading>Last Activity</TableHeading>
                <TableHeading>Owner</TableHeading>
              </tr>
            </thead>

            <tbody>
              {filteredDeals.length > 0 ? (
                filteredDeals.map((deal) => (
                  <tr
                    key={deal.id}
                    className="border-b border-[var(--portal-border)] last:border-b-0 hover:bg-[var(--portal-blue-pale)]/45"
                  >
                    <td className="px-6 py-5 align-top">
                      <div className="flex items-center gap-4">
                        {deal.heroMediaId ? (
                          <img
                            src={`/api/media/${deal.heroMediaId}`}
                            alt={deal.propertyName}
                            className="h-14 w-20 shrink-0 rounded-sm object-cover"
                          />
                        ) : (
                          <div className="h-14 w-20 shrink-0 rounded-sm bg-gradient-to-br from-[var(--portal-blue-pale)] via-[var(--portal-mist-4)] to-[var(--portal-blue-gray)]" />
                        )}

                        <div>
                          <Link
                            href={`/portal/deals/${deal.id}`}
                            className="font-serif text-lg font-light text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                          >
                            {deal.propertyName}
                          </Link>

                          <div className="mt-1 text-xs font-light text-black/45">
                            {deal.propertyLocation}
                            {deal.propertyDescriptor &&
                              ` · ${deal.propertyDescriptor}`}
                          </div>

                          {(deal.showingCount ||
                            deal.offerCount ||
                            deal.participantCount) && (
                            <div className="mt-1 text-xs font-light text-[var(--portal-navy-soft)]">
                              {dealSummary(deal)}
                            </div>
                          )}
                        </div>
                      </div>
                    </td>

                    <td className="px-6 py-5 align-top">
                      <div className="text-sm font-medium">
                        {deal.clientName}
                      </div>
                    </td>

                    <td className="px-6 py-5 align-top">
                      <span
                        className={[
                          "inline-flex rounded-full px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.1em]",
                          stageClasses(deal.stage),
                        ].join(" ")}
                      >
                        {stageLabel(deal.stage)}
                      </span>
                    </td>

                    <td className="px-6 py-5 align-top">
                      {deal.offerCount ? (
                        <>
                          <div className="text-sm font-light">
                            {formatCurrency(
                              deal.latestOfferAmount ??
                                deal.offerPrice ??
                                deal.listPrice
                            )}
                          </div>
                          <div className="mt-1 text-xs font-light text-black/40">
                            {deal.latestOfferStatus
                              ? deal.latestOfferStatus.charAt(0).toUpperCase() +
                                deal.latestOfferStatus.slice(1)
                              : "Latest offer"}
                          </div>
                        </>
                      ) : (
                        <>
                          <div className="text-sm font-light">
                            {formatCurrency(deal.offerPrice ?? deal.listPrice)}
                          </div>
                          <div className="mt-1 text-xs font-light text-black/40">
                            {deal.offerPrice ? "Current offer" : "List price"}
                          </div>
                        </>
                      )}
                    </td>

                    <td className="px-6 py-5 align-top">
                      <div className="text-sm font-light">
                        {deal.nextMilestone ?? "—"}
                      </div>

                      <div className="mt-1 text-xs font-light text-black/40">
                        {deal.nextMilestoneAt ?? ""}
                      </div>
                    </td>

                    <td className="px-6 py-5 align-top">
                      <div className="text-sm font-light">
                        {deal.lastActivity ?? "—"}
                      </div>

                      <div className="mt-1 text-xs font-light text-black/40">
                        {deal.lastActivityAt ?? ""}
                      </div>
                    </td>

                    <td className="px-6 py-5 align-top">
                      <div className="text-sm font-light">
                        {deal.owner}
                      </div>
                    </td>
                  </tr>
                ))
              ) : (
                <tr>
                  <td
                    colSpan={7}
                    className="px-6 py-12 text-center text-sm font-light text-black/40"
                  >
                    No deals found.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </section>

      <div className="mt-6 grid gap-6 2xl:grid-cols-[minmax(0,1.5fr)_minmax(320px,.5fr)]">
        <section className="rounded-sm border border-[var(--portal-border)] bg-white p-6">
          <div className="mb-6">
            <h2 className="font-serif text-2xl font-light">
              Pipeline
            </h2>

            <p className="mt-1 text-xs font-light text-black/40">
              The same portfolio grouped by deal stage.
            </p>
          </div>

          <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
            {stageOrder.map((stage) => {
              const stageDeals = deals.filter(
                (deal) => deal.stage === stage
              )

              return (
                <div
                  key={stage}
                  className="rounded-sm border border-[var(--portal-border)] bg-[var(--portal-blue-pale)]/55 p-4"
                >
                  <div className="flex items-center justify-between">
                    <div className="text-[10px] font-light uppercase tracking-[0.16em] text-[var(--portal-blue-gray)]">
                      {stageLabel(stage)}
                    </div>

                    <div className="text-xs font-light text-black/35">
                      {stageDeals.length}
                    </div>
                  </div>

                  <div className="mt-4 space-y-3">
                    {stageDeals.length > 0 ? (
                      stageDeals.map((deal) => (
                        <div
                          key={deal.id}
                          className="rounded-sm border border-[var(--portal-border)] bg-white p-4"
                        >
                          <div className="font-serif text-base font-light">
                            {deal.propertyName}
                          </div>

                          <div className="mt-1 text-xs font-light text-black/45">
                            {deal.clientName}
                          </div>

                          <div className="mt-3 text-sm font-light text-[var(--portal-navy)]">
                            {formatCurrency(
                              deal.offerPrice ?? deal.listPrice
                            )}
                          </div>
                        </div>
                      ))
                    ) : (
                      <div className="rounded-sm border border-dashed border-[var(--portal-border)] px-4 py-6 text-center text-xs font-light text-black/35">
                        No deals
                      </div>
                    )}
                  </div>
                </div>
              )
            })}
          </div>
        </section>

        <section className="rounded-sm border border-[var(--portal-border)] bg-[var(--portal-navy)] p-6 text-white">
          <div>
            <p className="text-[10px] font-light uppercase tracking-[0.2em] text-white/50">
              Deal Watchlist
            </p>

            <h2 className="mt-3 font-serif text-2xl font-light">
              Priority Closings
            </h2>
          </div>

          <div className="mt-6 divide-y divide-white/10">
            {closingSoon.length > 0 ? (
              closingSoon.map((deal) => (
                <div
                  key={deal.id}
                  className="py-5 first:pt-0 last:pb-0"
                >
                  <div className="font-serif text-lg font-light">
                    {deal.propertyName}
                  </div>

                  <div className="mt-1 text-xs font-light text-white/50">
                    {deal.clientName}
                  </div>

                  <div className="mt-4 text-sm font-light text-white/80">
                    {deal.closingDate}
                  </div>

                  <div className="mt-1 text-xs font-light text-white/45">
                    {stageLabel(deal.stage)}
                  </div>
                </div>
              ))
            ) : (
              <div className="text-sm font-light text-white/50">
                No scheduled closings.
              </div>
            )}
          </div>
        </section>
      </div>
    </div>
  )
}

function MetricCard({
  label,
  value,
  detail,
}: {
  label: string
  value: string
  detail: string
}) {
  return (
    <div className="rounded-sm border border-[var(--portal-border)] bg-white p-6">
      <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
        {label}
      </div>

      <div className="mt-4 font-serif text-3xl font-light text-[var(--portal-navy)]">
        {value}
      </div>

      <div className="mt-2 text-xs font-light text-black/40">
        {detail}
      </div>
    </div>
  )
}

function TableHeading({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <th className="px-6 py-4 text-left text-[10px] font-light uppercase tracking-[0.16em] text-[var(--portal-blue-gray)]">
      {children}
    </th>
  )
}