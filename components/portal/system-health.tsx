import type { SystemHealthSnapshot } from "@/db/system-health"

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

export function SystemHealth({
  health,
}: {
  health: SystemHealthSnapshot
}) {
  return (
    <div>
      <div className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Portal
        </p>

        <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
          System Health
        </h1>

        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Operational signals across intake, tasks, deals, properties and
          relationship data — read-only.
        </p>
      </div>

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          label="Needs Review"
          value={String(health.unresolvedIntakeCount)}
          detail="Unresolved intake submissions"
        />
        <MetricCard
          label="Open Tasks"
          value={String(health.openTaskCount)}
          detail={`${health.overdueTaskCount} overdue`}
        />
        <MetricCard
          label="Active Deals"
          value={String(health.activeDealCount)}
          detail={`${health.underContractCount} under contract`}
        />
        <MetricCard
          label="Active Properties"
          value={String(health.activePropertyCount)}
          detail="Active inventory"
        />
      </section>

      <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white p-6">
        <h2 className="font-serif text-2xl font-light">
          Recent Activity
        </h2>

        <div className="mt-4 grid gap-6 md:grid-cols-2">
          <Detail
            label="Last Interaction"
            value={health.recentInteractionAtLabel ?? "None recorded"}
          />
          <Detail
            label="Interactions (7 days)"
            value={String(health.interactionsLast7Days)}
          />
        </div>
      </section>

      <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white p-6">
        <h2 className="font-serif text-2xl font-light">
          Data Quality
        </h2>

        <p className="mt-1 text-xs font-light text-black/40">
          Signals worth reviewing, derived directly from the schema.
        </p>

        <div className="mt-6 grid gap-6 md:grid-cols-2 xl:grid-cols-4">
          <Detail
            label="Clients without email"
            value={String(health.personsWithoutEmailIdentity)}
          />
          <Detail
            label="Clients without phone"
            value={String(health.personsWithoutPhoneIdentity)}
          />
          <Detail
            label="Open tasks no due date"
            value={String(health.openTasksWithoutDueDate)}
          />
          <Detail
            label="Properties no hero image"
            value={String(health.activePropertiesWithoutHeroMedia)}
          />
        </div>
      </section>

      <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white p-6">
        <h2 className="font-serif text-2xl font-light">
          Transaction Data Quality
        </h2>

        <p className="mt-1 text-xs font-light text-black/40">
          Signals derivable from showings, offers, and participants.
        </p>

        <div className="mt-6 grid gap-6 md:grid-cols-2 xl:grid-cols-3">
          <Detail
            label="Completed showings missing completed_at"
            value={String(health.completedShowingsMissingCompletedAt)}
          />
          <Detail
            label="Scheduled showings missing scheduled_at"
            value={String(health.scheduledShowingsMissingScheduledAt)}
          />
          <Detail
            label="Active participants with ended_at"
            value={String(health.activeParticipantsWithEndedAt)}
          />
          <Detail
            label="Other participants missing role label"
            value={String(health.otherParticipantsMissingRoleLabel)}
          />
          <Detail
            label="Offers with cross-deal parent"
            value={String(health.offersWithCrossDealParent)}
          />
          <Detail
            label="Showings with deal/property mismatch"
            value={String(health.showingsWithDealPropertyMismatch)}
          />
        </div>
      </section>

      <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white p-6">
        <h2 className="font-serif text-2xl font-light">
          Write-Side Invariants
        </h2>

        <p className="mt-1 text-xs font-light text-black/40">
          Deterministic checks on invariants the listing/showing write services
          are expected to maintain.
        </p>

        <div className="mt-6 grid gap-6 md:grid-cols-2 xl:grid-cols-4">
          <Detail
            label="Completed showings no showing interaction"
            value={String(health.completedShowingsMissingShowingInteraction)}
          />
          <Detail
            label="Inactive participants without ended_at"
            value={String(health.inactiveParticipantsWithoutEndedAt)}
          />
          <Detail
            label="Public properties with multiple heroes"
            value={String(health.publicPropertiesWithMultipleHeroes)}
          />
          <Detail
            label="Hero media not an image"
            value={String(health.heroMediaNotImage)}
          />
          <Detail
            label="Role / account-type mismatches"
            value={String(health.accountTypeMismatchCount)}
          />
        </div>
      </section>

      <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white p-6">
        <h2 className="font-serif text-2xl font-light">
          Security Model
        </h2>

        <p className="mt-1 text-xs font-light text-black/40">
          Application security invariants (guarded pre-migration; 0 is healthy).
        </p>

        <div className="mt-6 grid gap-6 md:grid-cols-2 xl:grid-cols-3">
          <Detail
            label="Active users with no role"
            value={String(health.activeAppUsersWithoutRole)}
          />
          <Detail
            label="Auth identities → inactive user"
            value={String(health.authIdentityInactiveAppUser)}
          />
          <Detail
            label="Owner assignments"
            value={String(health.ownerAssignments)}
          />
          <Detail
            label="Multiple owners (informational)"
            value={String(health.multipleOwners)}
          />
          <Detail
            label="Auth identities without usable user"
            value={String(health.authIdentityWithoutUsableAppUser)}
          />
        </div>
      </section>
    </div>
  )
}

function Detail({
  label,
  value,
}: {
  label: string
  value: string
}) {
  return (
    <div>
      <div className="text-[10px] font-light uppercase tracking-[0.18em] text-black/35">
        {label}
      </div>

      <div className="mt-2 text-sm font-light leading-6 text-black/70">
        {value}
      </div>
    </div>
  )
}
