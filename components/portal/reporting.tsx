import type { ReportingSnapshot } from "@/db/reporting"

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
    <div className="portal-glass-panel portal-glass-panel-soft rounded-[var(--portal-panel-radius)] p-6">
      <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
        {label}
      </div>
      <div className="mt-4 font-serif text-3xl font-light text-[var(--portal-navy)]">
        {value}
      </div>
      <div className="mt-2 text-xs font-light text-black/40">{detail}</div>
    </div>
  )
}

function stageLabel(stage: string) {
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
    default:
      return stage
  }
}

function channelLabel(channel: string) {
  if (channel === "whatsapp") return "WhatsApp"
  return channel.charAt(0).toUpperCase() + channel.slice(1)
}

function statusLabel(status: string) {
  return status.charAt(0).toUpperCase() + status.slice(1)
}

function propertyStatusLabel(status: string) {
  switch (status) {
    case "coming_soon":
      return "Coming Soon"
    case "under_contract":
      return "Under Contract"
    case "off_market":
      return "Off Market"
    case "prospect":
      return "Prospect"
    case "sold":
      return "Sold"
    case "archived":
      return "Archived"
    default:
      return status.charAt(0).toUpperCase() + status.slice(1)
  }
}

function participantRoleLabel(role: string) {
  switch (role) {
    case "client":
      return "Client"
    case "owner":
      return "Owner"
    case "seller":
      return "Seller"
    default:
      return role.charAt(0).toUpperCase() + role.slice(1)
  }
}

function DistributionBar({
  label,
  count,
  total,
}: {
  label: string
  count: number
  total: number
}) {
  const percent = total > 0 ? Math.round((count / total) * 100) : 0

  return (
    <div>
      <div className="mb-1 flex items-center justify-between">
        <div className="text-xs font-light uppercase tracking-[0.12em] text-black/50">
          {label}
        </div>
        <div className="text-xs font-light text-black/40">{count}</div>
      </div>
      <div className="h-1.5 overflow-hidden bg-[var(--portal-blue-pale)]">
        <div
          className="h-full bg-[var(--portal-navy)]"
          style={{ width: `${percent}%` }}
        />
      </div>
    </div>
  )
}

export function Reporting({
  snapshot,
}: {
  snapshot: ReportingSnapshot
}) {
  return (
    <div>
      <div className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Portal
        </p>

        <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
          Reporting
        </h1>

        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Boutique operational metrics drawn directly from the CRM — no
          forecasts, no scoring.
        </p>
      </div>

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          label="Active Clients"
          value={String(snapshot.activeClientCount)}
          detail="Non-archived people"
        />
        <MetricCard
          label="Active Deals"
          value={String(snapshot.activeDealCount)}
          detail="Deals not closed"
        />
        <MetricCard
          label="Open Tasks"
          value={String(snapshot.openTaskCount)}
          detail={`${snapshot.overdueTaskCount} overdue`}
        />
        <MetricCard
          label="Needs Review"
          value={String(snapshot.unresolvedNeedsReviewCount)}
          detail="Unresolved intake submissions"
        />
      </section>

      <section className="mt-6 grid gap-6 2xl:grid-cols-2">
        <section className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
          <h2 className="font-serif text-2xl font-light">Deal Stage Distribution</h2>
          <div className="mt-5 space-y-4">
            {snapshot.dealStageDistribution.length > 0 ? (
              snapshot.dealStageDistribution.map((item) => (
                <DistributionBar
                  key={item.stage}
                  label={stageLabel(item.stage)}
                  count={item.count}
                  total={snapshot.totalDealCount}
                />
              ))
            ) : (
              <p className="text-sm font-light text-black/40">No deals on record.</p>
            )}
          </div>
        </section>

        <section className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
          <h2 className="font-serif text-2xl font-light">Client Roles</h2>
          <div className="mt-5 space-y-4">
            {snapshot.clientRoleDistribution.length > 0 ? (
              snapshot.clientRoleDistribution.map((item) => (
                <DistributionBar
                  key={item.role}
                  label={item.role.charAt(0).toUpperCase() + item.role.slice(1)}
                  count={item.count}
                  total={snapshot.activeClientCount}
                />
              ))
            ) : (
              <p className="text-sm font-light text-black/40">No people on record.</p>
            )}
          </div>
        </section>
      </section>

      <section className="mt-6 grid gap-6 2xl:grid-cols-2">
        <section className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
          <h2 className="font-serif text-2xl font-light">Activity</h2>
          <div className="mt-4 grid gap-4 md:grid-cols-2">
            <MetricCard
              label="Interactions (7 days)"
              value={String(snapshot.interactionsLast7Days)}
              detail="Recent deterministic window"
            />
            <MetricCard
              label="Interactions (all time)"
              value={String(snapshot.totalInteractions)}
              detail="Total on record"
            />
          </div>
        </section>

        <section className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
          <h2 className="font-serif text-2xl font-light">Interactions by Channel</h2>
          <div className="mt-5 space-y-4">
            {snapshot.interactionByChannel.length > 0 ? (
              snapshot.interactionByChannel.map((item) => (
                <DistributionBar
                  key={item.channel}
                  label={channelLabel(item.channel)}
                  count={item.count}
                  total={snapshot.totalInteractions}
                />
              ))
            ) : (
              <p className="text-sm font-light text-black/40">No interactions recorded.</p>
            )}
          </div>
        </section>
      </section>

      <section className="mt-6 portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
        <h2 className="font-serif text-2xl font-light">Listing & Seller</h2>
        <p className="mt-1 text-xs font-light text-black/40">
          Brokerage inventory and listing-health signals from canonical data.
        </p>

        <div className="mt-4 grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          <MetricCard
            label="Active Properties"
            value={String(snapshot.activePropertyCount)}
            detail="Active inventory"
          />
          <MetricCard
            label="With Seller"
            value={String(snapshot.propertiesWithSellerCount)}
            detail="Listings with seller_person_id"
          />
          <MetricCard
            label="Public Missing Hero"
            value={String(snapshot.publicPropertiesMissingHero)}
            detail="Active public listings without hero"
          />
          <MetricCard
            label="Public No Media"
            value={String(snapshot.publicPropertiesNoMedia)}
            detail="Active public listings with no media"
          />
        </div>

        <div className="mt-6 grid gap-6 2xl:grid-cols-2">
          <div>
            <h3 className="text-xs font-light uppercase tracking-[0.2em] text-black/40">
              Listings by Status
            </h3>
            <div className="mt-4 space-y-4">
              {snapshot.listingsByStatus.length > 0 ? (
                snapshot.listingsByStatus.map((item) => (
                  <DistributionBar
                    key={item.status}
                    label={propertyStatusLabel(item.status)}
                    count={item.count}
                    total={snapshot.listingsByStatus.reduce(
                      (sum, row) => sum + row.count,
                      0,
                    )}
                  />
                ))
              ) : (
                <p className="text-sm font-light text-black/40">
                  No properties on record.
                </p>
              )}
            </div>
          </div>

          <div>
            <h3 className="text-xs font-light uppercase tracking-[0.2em] text-black/40">
              Enquiries by Property
            </h3>
            <div className="mt-4 space-y-4">
              {snapshot.enquiriesByProperty.length > 0 ? (
                snapshot.enquiriesByProperty.map((item) => (
                  <DistributionBar
                    key={item.propertyName}
                    label={item.propertyName}
                    count={item.count}
                    total={snapshot.enquiriesByProperty.reduce(
                      (sum, row) => sum + row.count,
                      0,
                    )}
                  />
                ))
              ) : (
                <p className="text-sm font-light text-black/40">
                  No website enquiries on record.
                </p>
              )}
            </div>
          </div>
        </div>

        <div className="mt-6">
          <h3 className="text-xs font-light uppercase tracking-[0.2em] text-black/40">
            Showings by Property
          </h3>
          <div className="mt-4 grid gap-6 md:grid-cols-2 xl:grid-cols-3">
            {snapshot.showingsByProperty.length > 0 ? (
              snapshot.showingsByProperty.map((item) => (
                <div
                  key={item.propertyName}
                  className="rounded-sm border border-[var(--portal-border)] bg-[var(--portal-blue-pale)]/40 px-4 py-3"
                >
                  <div className="text-xs font-light text-black/60">
                    {item.propertyName}
                  </div>
                  <div className="mt-1 font-serif text-xl font-light">
                    {item.count}
                  </div>
                </div>
              ))
            ) : (
              <p className="text-sm font-light text-black/40">
                No showings on record.
              </p>
            )}
          </div>
        </div>
      </section>

      <section className="mt-6 grid gap-6 2xl:grid-cols-2">
        <section className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
          <h2 className="font-serif text-2xl font-light">Showings</h2>
          <div className="mt-4 grid gap-4 md:grid-cols-2">
            <MetricCard
              label="Completed (30 days)"
              value={String(snapshot.recentCompletedShowings)}
              detail="Recent completed showings"
            />
          </div>
          <div className="mt-5 space-y-4">
            {snapshot.showingByStatus.length > 0 ? (
              snapshot.showingByStatus.map((item) => (
                <DistributionBar
                  key={item.status}
                  label={statusLabel(item.status)}
                  count={item.count}
                  total={snapshot.showingByStatus.reduce(
                    (sum, row) => sum + row.count,
                    0,
                  )}
                />
              ))
            ) : (
              <p className="text-sm font-light text-black/40">No showings on record.</p>
            )}
          </div>
        </section>

        <section className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
          <h2 className="font-serif text-2xl font-light">Offers</h2>
          <div className="mt-4 grid gap-4 md:grid-cols-2">
            <MetricCard
              label="Deals with Offers"
              value={String(snapshot.dealsWithOffers)}
              detail="Distinct deals"
            />
          </div>
          <div className="mt-5 space-y-4">
            {snapshot.offersByStatus.length > 0 ? (
              snapshot.offersByStatus.map((item) => (
                <DistributionBar
                  key={item.status}
                  label={statusLabel(item.status)}
                  count={item.count}
                  total={snapshot.offersByStatus.reduce(
                    (sum, row) => sum + row.count,
                    0,
                  )}
                />
              ))
            ) : (
              <p className="text-sm font-light text-black/40">No offers on record.</p>
            )}
          </div>
          <h3 className="mt-6 text-xs font-light uppercase tracking-[0.2em] text-black/40">
            Participants
          </h3>
          <div className="mt-4 space-y-4">
            {snapshot.participantRoleDistribution.length > 0 ? (
              snapshot.participantRoleDistribution.map((item) => (
                <DistributionBar
                  key={item.role}
                  label={participantRoleLabel(item.role)}
                  count={item.count}
                  total={snapshot.participantRoleDistribution.reduce(
                    (sum, row) => sum + row.count,
                    0,
                  )}
                />
              ))
            ) : (
              <p className="text-sm font-light text-black/40">No participants on record.</p>
            )}
          </div>
        </section>
      </section>
    </div>
  )
}
