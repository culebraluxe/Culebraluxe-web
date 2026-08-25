import Link from "next/link"

import type { IdentityQualitySnapshot } from "@/db/identity-quality"
import {
  PortalTable,
  PortalTableBody,
  PortalTableCell,
  PortalTableHead,
  PortalTableHeader,
  PortalTableRow,
} from "@/components/portal/ui/portal-table"

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

export function IdentityQuality({
  snapshot,
}: {
  snapshot: IdentityQualitySnapshot
}) {
  return (
    <div>
      <div className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Operations
        </p>

        <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
          Identity & Contact Quality
        </h1>

        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Read-only view of contact coverage and identity hygiene across
          canonical people.
        </p>
      </div>

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          label="People"
          value={String(snapshot.totalPeople)}
          detail="Non-archived people"
        />
        <MetricCard
          label="No Identity"
          value={String(snapshot.peopleWithNoIdentity)}
          detail="People with zero identities"
        />
        <MetricCard
          label="No Email"
          value={String(snapshot.peopleWithoutEmail)}
          detail="Missing an email identity"
        />
        <MetricCard
          label="No Phone"
          value={String(snapshot.peopleWithoutPhone)}
          detail="Missing a phone identity"
        />
      </section>

      <section className="mt-6 portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
        <h2 className="font-serif text-2xl font-light">Coverage Gaps</h2>

        <div className="mt-4 grid gap-4 md:grid-cols-3">
          <MetricCard
            label="Email, no primary"
            value={String(snapshot.peopleWithoutPrimaryEmail)}
            detail="Has email but none marked primary"
          />
          <MetricCard
            label="Phone, no primary"
            value={String(snapshot.peopleWithoutPrimaryPhone)}
            detail="Has phone but none marked primary"
          />
          <MetricCard
            label="Identity types"
            value={String(snapshot.identityCountByType.length)}
            detail="Distinct identity types on file"
          />
        </div>

        <div className="mt-6 grid gap-4 md:grid-cols-3">
          {snapshot.identityCountByType.map((item) => (
            <div
              key={item.identityType}
              className="portal-glass-panel portal-glass-panel-soft rounded-[var(--portal-panel-radius)] p-5"
            >
              <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
                {item.identityType}
              </div>
              <div className="mt-3 font-serif text-2xl font-light">{item.count}</div>
            </div>
          ))}
        </div>
      </section>

      <div className="mt-6 grid gap-6 2xl:grid-cols-2">
        <section className="portal-glass-panel rounded-[var(--portal-panel-radius)]">
          <div className="flex items-center justify-between border-b border-[var(--portal-border)] px-6 py-5">
            <div>
              <h2 className="font-serif text-2xl font-light">Malformed Identities</h2>
              <p className="mt-1 text-xs font-light text-black/40">
                Flagged using the existing strict email / E.164 rules.
              </p>
            </div>
            <span className="text-xs font-light text-black/35">
              {snapshot.malformedIdentities.length}
            </span>
          </div>
          {snapshot.malformedIdentities.length > 0 ? (
            <>
              <div className="hidden md:block">
                <PortalTable>
                  <PortalTableHead>
                    <PortalTableRow className="hover:bg-transparent">
                      <PortalTableHeader>Person</PortalTableHeader>
                      <PortalTableHeader>Identity</PortalTableHeader>
                      <PortalTableHeader>Issue</PortalTableHeader>
                    </PortalTableRow>
                  </PortalTableHead>
                  <PortalTableBody>
                    {snapshot.malformedIdentities.map((item) => (
                      <PortalTableRow key={`${item.personId}-${item.identityType}-${item.value}`}>
                        <PortalTableCell>
                          <Link
                            href={`/portal/clients/${item.personId}`}
                            className="text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]"
                          >
                            {item.personName}
                          </Link>
                        </PortalTableCell>
                        <PortalTableCell className="text-xs font-light text-black/45">
                          {item.identityType}: {item.value}
                        </PortalTableCell>
                        <PortalTableCell className="text-[10px] font-light uppercase tracking-[0.1em] text-[var(--portal-archive)]">
                          {item.issue}
                        </PortalTableCell>
                      </PortalTableRow>
                    ))}
                  </PortalTableBody>
                </PortalTable>
              </div>
              <div className="divide-y divide-[var(--portal-border)] md:hidden">
                {snapshot.malformedIdentities.map((item) => (
                  <article key={`${item.personId}-${item.identityType}-${item.value}`} className="space-y-2 px-4 py-4">
                    <Link
                      href={`/portal/clients/${item.personId}`}
                      className="text-sm font-medium text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]"
                    >
                      {item.personName}
                    </Link>
                    <dl className="text-xs">
                      <div>
                        <dt className="sr-only">Identity</dt>
                        <dd className="font-light text-black/45">
                          {item.identityType}: {item.value}
                        </dd>
                      </div>
                      <div>
                        <dt className="sr-only">Issue</dt>
                        <dd className="mt-1 text-[10px] font-light uppercase tracking-[0.1em] text-[var(--portal-archive)]">
                          {item.issue}
                        </dd>
                      </div>
                    </dl>
                  </article>
                ))}
              </div>
            </>
          ) : (
            <div className="px-6 py-10 text-sm font-light text-black/40">
              No malformed identities found.
            </div>
          )}
        </section>

        <section className="portal-glass-panel rounded-[var(--portal-panel-radius)]">
          <div className="flex items-center justify-between border-b border-[var(--portal-border)] px-6 py-5">
            <div>
              <h2 className="font-serif text-2xl font-light">Weak Contact Coverage</h2>
              <p className="mt-1 text-xs font-light text-black/40">
                Active work or deals but no email or phone on file.
              </p>
            </div>
            <span className="text-xs font-light text-black/35">
              {snapshot.weakCoverage.length}
            </span>
          </div>
          {snapshot.weakCoverage.length > 0 ? (
            <>
              <div className="hidden md:block">
                <PortalTable>
                  <PortalTableHead>
                    <PortalTableRow className="hover:bg-transparent">
                      <PortalTableHeader>Person</PortalTableHeader>
                      <PortalTableHeader>Role / Status</PortalTableHeader>
                      <PortalTableHeader className="text-right">Active Deals</PortalTableHeader>
                      <PortalTableHeader className="text-right">Open Tasks</PortalTableHeader>
                    </PortalTableRow>
                  </PortalTableHead>
                  <PortalTableBody>
                    {snapshot.weakCoverage.map((person) => (
                      <PortalTableRow key={person.id}>
                        <PortalTableCell>
                          <Link
                            href={`/portal/clients/${person.id}`}
                            className="font-serif text-base font-light text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]"
                          >
                            {person.displayName}
                          </Link>
                        </PortalTableCell>
                        <PortalTableCell className="text-xs font-light text-black/45">
                          {person.role} · {person.status}
                        </PortalTableCell>
                        <PortalTableCell className="text-right text-sm font-light tabular-nums">
                          {person.activeDealCount}
                        </PortalTableCell>
                        <PortalTableCell className="text-right text-sm font-light tabular-nums">
                          {person.openTaskCount}
                        </PortalTableCell>
                      </PortalTableRow>
                    ))}
                  </PortalTableBody>
                </PortalTable>
              </div>
              <div className="divide-y divide-[var(--portal-border)] md:hidden">
                {snapshot.weakCoverage.map((person) => (
                  <article key={person.id} className="space-y-3 px-4 py-4">
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <Link
                          href={`/portal/clients/${person.id}`}
                          className="font-serif text-lg font-light text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]"
                        >
                          {person.displayName}
                        </Link>
                        <p className="mt-0.5 text-xs font-light text-black/45">
                          {person.role} · {person.status}
                        </p>
                      </div>
                    </div>
                    <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
                      <div>
                        <dt className="font-light uppercase tracking-[0.12em] text-black/35">Active deals</dt>
                        <dd className="mt-1 font-light text-black/65">{person.activeDealCount}</dd>
                      </div>
                      <div>
                        <dt className="font-light uppercase tracking-[0.12em] text-black/35">Open tasks</dt>
                        <dd className="mt-1 font-light text-black/65">{person.openTaskCount}</dd>
                      </div>
                    </dl>
                  </article>
                ))}
              </div>
            </>
          ) : (
            <div className="px-6 py-10 text-sm font-light text-black/40">
              No weak-coverage relationships.
            </div>
          )}
        </section>
      </div>

      <div className="mt-6 portal-glass-panel portal-glass-panel-soft rounded-[var(--portal-panel-radius)] p-6">
        <h2 className="font-serif text-2xl font-light">Duplicate Identities</h2>
        <p className="mt-3 text-sm font-light leading-6 text-black/55">
          {snapshot.exactDuplicateCheck.note}
        </p>
      </div>
    </div>
  )
}
