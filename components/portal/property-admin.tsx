import Link from "next/link"

import type { PropertyAdminRow } from "@/db/property-admin"
import { PropertyArchiveButton } from "@/components/portal/write/property-archive-button"
import {
  PortalTable,
  PortalTableBody,
  PortalTableCell,
  PortalTableHead,
  PortalTableHeader,
  PortalTableRow,
} from "@/components/portal/ui/portal-table"

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

const QUALITY_LABELS: Array<{
  flag: keyof PropertyAdminRow["dataQuality"]
  label: string
}> = [
  { flag: "missingSlug", label: "no slug" },
  { flag: "missingPrice", label: "no price" },
  { flag: "missingLocation", label: "no location" },
  { flag: "missingDescription", label: "no description" },
  { flag: "missingBedsBaths", label: "no beds/baths" },
  { flag: "noMedia", label: "no media" },
  { flag: "missingHero", label: "no hero" },
  { flag: "malformedLotUnits", label: "lot units" },
]

function qualityIssues(row: PropertyAdminRow) {
  return QUALITY_LABELS.filter((item) => row.dataQuality[item.flag]).map(
    (item) => item.label,
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
          Operational listing index: scan status, price, location, media
          completeness, and data-quality issues, then open the per-listing
          workspace to maintain it.
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

        <div className="hidden md:block">
          <PortalTable>
            <PortalTableHead>
              <PortalTableRow className="hover:bg-transparent">
                <PortalTableHeader>Property</PortalTableHeader>
                <PortalTableHeader>Status</PortalTableHeader>
                <PortalTableHeader>Featured</PortalTableHeader>
                <PortalTableHeader>Price</PortalTableHeader>
                <PortalTableHeader>Location</PortalTableHeader>
                <PortalTableHeader>Beds / Baths / Area</PortalTableHeader>
                <PortalTableHeader>Seller</PortalTableHeader>
                <PortalTableHeader>Hero</PortalTableHeader>
                <PortalTableHeader>Img / Vid / Doc</PortalTableHeader>
                <PortalTableHeader>Data quality</PortalTableHeader>
                <PortalTableHeader>Archived</PortalTableHeader>
                <PortalTableHeader>Actions</PortalTableHeader>
              </PortalTableRow>
            </PortalTableHead>
            <PortalTableBody>
              {rows.length > 0 ? (
                rows.map((row) => (
                  <PortalTableRow key={row.id}>
                    <PortalTableCell>
                      <Link
                        href={`/portal/property-admin/${row.id}`}
                        className="font-serif text-lg font-light text-[var(--portal-navy-soft)] underline-offset-2 hover:underline"
                      >
                        {row.name}
                      </Link>
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
                      <Link
                        href={`/portal/property-admin/${row.id}`}
                        className="mt-2 inline-block text-xs font-light text-black/50 underline-offset-2 hover:text-[var(--portal-navy)] hover:underline"
                      >
                        Manage listing →
                      </Link>
                    </PortalTableCell>
                    <PortalTableCell className="text-sm font-light">
                      {statusLabel(row.status)}
                    </PortalTableCell>
                    <PortalTableCell className="text-sm font-light">
                      {row.featured ? "Yes" : "—"}
                    </PortalTableCell>
                    <PortalTableCell className="text-sm font-light">
                      {formatCurrency(row.listPrice)}
                    </PortalTableCell>
                    <PortalTableCell className="text-sm font-light">
                      {row.location ?? "—"}
                    </PortalTableCell>
                    <PortalTableCell className="text-sm font-light">
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
                    </PortalTableCell>
                    <PortalTableCell className="text-sm font-light">
                      {row.sellerName ?? "—"}
                    </PortalTableCell>
                    <PortalTableCell className="text-sm font-light">
                      {row.heroMediaId ? (
                        <span className="text-[var(--portal-success)]">✓</span>
                      ) : (
                        <span className="text-[var(--portal-archive)]">Missing</span>
                      )}
                    </PortalTableCell>
                    <PortalTableCell className="text-sm font-light">
                      {row.imageCount} / {row.videoCount} / {row.documentCount}
                    </PortalTableCell>
                    <PortalTableCell className="text-sm font-light">
                      {row.issueCount === 0 ? (
                        <span className="text-[var(--portal-success)]">✓ Complete</span>
                      ) : (
                        <div>
                          <span className="font-light text-[var(--portal-archive)]">
                            {row.issueCount} issue{row.issueCount === 1 ? "" : "s"}
                          </span>
                          <div className="mt-1 max-w-[220px] text-[11px] leading-4 text-black/45">
                            {qualityIssues(row).join(" · ")}
                          </div>
                        </div>
                      )}
                    </PortalTableCell>
                    <PortalTableCell className="text-sm font-light">
                      {row.archived ? "Yes" : "—"}
                    </PortalTableCell>
                    <PortalTableCell>
                      <PropertyArchiveButton
                        propertyId={row.id}
                        name={row.name}
                        archived={row.archived}
                      />
                    </PortalTableCell>
                  </PortalTableRow>
                ))
              ) : (
                <PortalTableRow>
                  <PortalTableCell
                    colSpan={12}
                    className="py-12 text-center text-black/40"
                  >
                    No properties on file.
                  </PortalTableCell>
                </PortalTableRow>
              )}
            </PortalTableBody>
          </PortalTable>
        </div>

        <div className="divide-y divide-[var(--portal-border)] md:hidden">
          {rows.length > 0 ? (
            rows.map((row) => (
              <article key={row.id} className="space-y-3 px-4 py-4">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <Link
                      href={`/portal/property-admin/${row.id}`}
                      className="font-serif text-lg font-light text-[var(--portal-navy-soft)] underline-offset-2 hover:underline"
                    >
                      {row.name}
                    </Link>
                    {row.propertyType && (
                      <p className="mt-0.5 text-xs font-light text-black/40">{row.propertyType}</p>
                    )}
                  </div>
                  <PropertyArchiveButton
                    propertyId={row.id}
                    name={row.name}
                    archived={row.archived}
                  />
                </div>
                <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
                  <div>
                    <dt className="font-light uppercase tracking-[0.12em] text-black/35">Status</dt>
                    <dd className="mt-1 font-light text-black/65">{statusLabel(row.status)}</dd>
                  </div>
                  <div>
                    <dt className="font-light uppercase tracking-[0.12em] text-black/35">Price</dt>
                    <dd className="mt-1 font-light text-black/65">{formatCurrency(row.listPrice)}</dd>
                  </div>
                  <div className="col-span-2">
                    <dt className="font-light uppercase tracking-[0.12em] text-black/35">Location</dt>
                    <dd className="mt-1 font-light text-black/65">{row.location ?? "—"}</dd>
                  </div>
                  <div className="col-span-2">
                    <dt className="font-light uppercase tracking-[0.12em] text-black/35">Beds / Baths / Area</dt>
                    <dd className="mt-1 font-light text-black/65">
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
                    </dd>
                  </div>
                  <div>
                    <dt className="font-light uppercase tracking-[0.12em] text-black/35">Hero</dt>
                    <dd className="mt-1 font-light text-black/65">
                      {row.heroMediaId ? (
                        <span className="text-[var(--portal-success)]">✓</span>
                      ) : (
                        <span className="text-[var(--portal-archive)]">Missing</span>
                      )}
                    </dd>
                  </div>
                  <div>
                    <dt className="font-light uppercase tracking-[0.12em] text-black/35">Media</dt>
                    <dd className="mt-1 font-light text-black/65">
                      {row.imageCount} / {row.videoCount} / {row.documentCount}
                    </dd>
                  </div>
                  <div>
                    <dt className="font-light uppercase tracking-[0.12em] text-black/35">Featured</dt>
                    <dd className="mt-1 font-light text-black/65">{row.featured ? "Yes" : "—"}</dd>
                  </div>
                  <div>
                    <dt className="font-light uppercase tracking-[0.12em] text-black/35">Archived</dt>
                    <dd className="mt-1 font-light text-black/65">{row.archived ? "Yes" : "—"}</dd>
                  </div>
                  <div className="col-span-2">
                    <dt className="font-light uppercase tracking-[0.12em] text-black/35">Data quality</dt>
                    <dd className="mt-1 font-light text-black/65">
                      {row.issueCount === 0
                        ? "✓ Complete"
                        : `${row.issueCount} issue${row.issueCount === 1 ? "" : "s"}: ${qualityIssues(row).join(" · ")}`}
                    </dd>
                  </div>
                </dl>
              </article>
            ))
          ) : (
            <p className="px-4 py-10 text-center text-sm font-light text-black/40">
              No properties on file.
            </p>
          )}
        </div>
      </section>
    </div>
  )
}
