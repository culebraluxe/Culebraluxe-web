import Link from "next/link"

import type { ClientAdminRow } from "@/db/client-admin"
import { ClientArchiveButton } from "@/components/portal/write/client-archive-button"

function roleLabel(role: string) {
  if (role === "both") return "Buyer & Seller"
  return role.charAt(0).toUpperCase() + role.slice(1)
}

function statusLabel(status: string) {
  return status.charAt(0).toUpperCase() + status.slice(1)
}

function TableHeading({ children }: { children: React.ReactNode }) {
  return (
    <th className="px-4 py-3 text-left text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
      {children}
    </th>
  )
}

export function ClientAdmin({
  rows,
}: {
  rows: ClientAdminRow[]
}) {
  return (
    <section className="mt-10 rounded-sm border border-[var(--portal-border)] bg-white">
      <div className="flex items-center justify-between border-b border-[var(--portal-border)] px-6 py-5">
        <div>
          <h2 className="font-serif text-2xl font-light">Client Administration</h2>
          <p className="mt-1 text-xs font-light text-black/40">
            Read-only operational view of people, contact coverage, and activity.
          </p>
        </div>
        <span className="text-xs font-light text-black/35">{rows.length} people</span>
      </div>

      <div className="overflow-x-auto">
        <table className="w-full min-w-[1100px] border-collapse">
          <thead>
            <tr className="border-b border-[var(--portal-border)] bg-[var(--portal-blue-pale)]">
              <TableHeading>Person</TableHeading>
              <TableHeading>Role / Status</TableHeading>
              <TableHeading>Email</TableHeading>
              <TableHeading>Phone</TableHeading>
              <TableHeading>Assigned</TableHeading>
              <TableHeading>Last Interaction</TableHeading>
              <TableHeading>Open Tasks</TableHeading>
              <TableHeading>Active Deals</TableHeading>
              <TableHeading>Interests</TableHeading>
              <TableHeading>Actions</TableHeading>
            </tr>
          </thead>
          <tbody>
            {rows.length > 0 ? (
              rows.map((row) => (
                <tr
                  key={row.id}
                  className="border-b border-[var(--portal-border)] last:border-b-0 hover:bg-[var(--portal-blue-pale)]/40"
                >
                  <td className="px-4 py-4 align-top">
                    <Link
                      href={`/portal/clients/${row.id}`}
                      className="font-serif text-lg font-light text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                    >
                      {row.displayName}
                    </Link>
                    {row.location && (
                      <div className="mt-1 text-xs font-light text-black/40">
                        {row.location}
                      </div>
                    )}
                  </td>
                  <td className="px-4 py-4 align-top">
                    <div className="text-sm font-medium">{roleLabel(row.role)}</div>
                    <div className="mt-1 text-xs font-light text-black/45">
                      {statusLabel(row.status)}
                    </div>
                  </td>
                  <td className="px-4 py-4 align-top">
                    <div className="text-sm font-light">
                      {row.primaryEmail ?? "—"}
                    </div>
                    {!row.primaryEmail && (
                      <div className="mt-1 text-[10px] font-light uppercase tracking-[0.1em] text-[#8a4b2a]">
                        No email
                      </div>
                    )}
                  </td>
                  <td className="px-4 py-4 align-top">
                    <div className="text-sm font-light">{row.primaryPhone ?? "—"}</div>
                    {!row.primaryPhone && (
                      <div className="mt-1 text-[10px] font-light uppercase tracking-[0.1em] text-[#8a4b2a]">
                        No phone
                      </div>
                    )}
                  </td>
                  <td className="px-4 py-4 align-top text-sm font-light">
                    {row.assignedAgent ?? "—"}
                  </td>
                  <td className="px-4 py-4 align-top text-sm font-light">
                    {row.lastInteractionLabel ?? "—"}
                  </td>
                  <td className="px-4 py-4 align-top text-sm font-light">
                    {row.openTaskCount}
                  </td>
                  <td className="px-4 py-4 align-top text-sm font-light">
                    {row.activeDealCount}
                  </td>
                  <td className="px-4 py-4 align-top text-sm font-light">
                    {row.interestCount}
                  </td>
                  <td className="px-4 py-4 align-top">
                    <ClientArchiveButton
                      personId={row.id}
                      displayName={row.displayName}
                    />
                  </td>
                </tr>
              ))
            ) : (
              <tr>
                <td
                  colSpan={10}
                  className="px-4 py-12 text-center text-sm font-light text-black/40"
                >
                  No people on file.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  )
}
