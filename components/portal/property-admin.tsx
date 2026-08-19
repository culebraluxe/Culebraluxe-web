import Link from "next/link"

import type { PropertyAdminRow } from "@/db/property-admin"

function formatCurrency(value?: number | null) {
  if (!value) return "—"

  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0,
  }).format(value)
}

function statusLabel(status: string) {
  switch (status) {
    case "coming_soon":
      return "Coming Soon"
    case "under_contract":
      return "Under Contract"
    case "off_market":
      return "Off Market"
    case "archived":
      return "Archived"
    case "prospect":
      return "Prospect"
    default:
      return status.charAt(0).toUpperCase() + status.slice(1)
  }
}

function TableHeading({ children }: { children: React.ReactNode }) {
  return (
    <th className="px-4 py-3 text-left text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
      {children}
    </th>
  )
}

export function PropertyAdmin({
  rows,
}: {
  rows: PropertyAdminRow[]
}) {
  return (
    <div>
      <div className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Operations
        </p>

        <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
          Property Administration
        </h1>

        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Read-only brokerage inventory: listings, media completeness, and
          archived state.
        </p>
      </div>

      <section className="overflow-hidden rounded-sm border border-[var(--portal-border)] bg-white">
        <div className="flex items-center justify-between border-b border-[var(--portal-border)] px-6 py-5">
          <div>
            <h2 className="font-serif text-2xl font-light">Inventory</h2>
            <p className="mt-1 text-xs font-light text-black/40">
              Active and archived properties.
            </p>
          </div>
          <span className="text-xs font-light text-black/35">{rows.length} properties</span>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full min-w-[1200px] border-collapse">
            <thead>
              <tr className="border-b border-[var(--portal-border)] bg-[var(--portal-blue-pale)]">
                <TableHeading>Property</TableHeading>
                <TableHeading>Status</TableHeading>
                <TableHeading>Featured</TableHeading>
                <TableHeading>Price</TableHeading>
                <TableHeading>Location</TableHeading>
                <TableHeading>Beds / Baths / Area</TableHeading>
                <TableHeading>Seller</TableHeading>
                <TableHeading>Hero</TableHeading>
                <TableHeading>Img / Vid / Doc</TableHeading>
                <TableHeading>Archived</TableHeading>
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
                      <div className="font-serif text-lg font-light">
                        {row.name}
                      </div>
                      {row.slug ? (
                        <Link
                          href={`/properties/${row.slug}`}
                          className="mt-1 inline-block text-xs font-light text-[var(--portal-navy-soft)] underline-offset-2 hover:underline"
                        >
                          View public listing
                        </Link>
                      ) : (
                        <div className="mt-1 text-xs font-light text-black/35">
                          No slug
                        </div>
                      )}
                      {row.propertyType && (
                        <div className="mt-1 text-xs font-light text-black/40">
                          {row.propertyType}
                        </div>
                      )}
                    </td>
                    <td className="px-4 py-4 align-top text-sm font-light">
                      {statusLabel(row.status)}
                    </td>
                    <td className="px-4 py-4 align-top text-sm font-light">
                      {row.featured ? "Yes" : "—"}
                    </td>
                    <td className="px-4 py-4 align-top text-sm font-light">
                      {formatCurrency(row.listPrice)}
                    </td>
                    <td className="px-4 py-4 align-top text-sm font-light">
                      {row.location ?? "—"}
                    </td>
                    <td className="px-4 py-4 align-top text-sm font-light">
                      {row.bedrooms != null || row.bathrooms != null || row.squareFeet != null
                        ? [
                            row.bedrooms != null ? `${row.bedrooms} bed` : null,
                            row.bathrooms != null ? `${row.bathrooms} bath` : null,
                            row.squareFeet != null
                              ? `${row.squareFeet.toLocaleString("en-US")} SF`
                              : null,
                          ]
                            .filter(Boolean)
                            .join(" · ")
                        : "—"}
                    </td>
                    <td className="px-4 py-4 align-top text-sm font-light">
                      {row.sellerName ?? "—"}
                    </td>
                    <td className="px-4 py-4 align-top text-sm font-light">
                      {row.heroMediaId ? (
                        <span className="text-[#40584b]">✓</span>
                      ) : (
                        <span className="text-[#8a4b2a]">Missing</span>
                      )}
                    </td>
                    <td className="px-4 py-4 align-top text-sm font-light">
                      {row.imageCount} / {row.videoCount} / {row.documentCount}
                    </td>
                    <td className="px-4 py-4 align-top text-sm font-light">
                      {row.archived ? "Yes" : "—"}
                    </td>
                  </tr>
                ))
              ) : (
                <tr>
                  <td
                    colSpan={10}
                    className="px-4 py-12 text-center text-sm font-light text-black/40"
                  >
                    No properties on file.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  )
}
