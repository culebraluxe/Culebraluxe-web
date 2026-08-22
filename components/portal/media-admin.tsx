import type { MediaAdminSnapshot } from "@/db/media-admin"

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
      <div className="mt-2 text-xs font-light text-black/40">{detail}</div>
    </div>
  )
}

function TableHeading({ children }: { children: React.ReactNode }) {
  return (
    <th className="px-4 py-3 text-left text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
      {children}
    </th>
  )
}

export function MediaAdmin({
  snapshot,
}: {
  snapshot: MediaAdminSnapshot
}) {
  return (
    <div>
      <div className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Operations
        </p>

        <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
          Media & Documents
        </h1>

        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Read-only audit of media, documents, and property coverage.
        </p>
      </div>

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          label="Total Media"
          value={String(snapshot.totalMedia)}
          detail="All rows in the media table"
        />
        <MetricCard
          label="Unlinked Media"
          value={String(snapshot.totalUnlinked)}
          detail="Not attached to a property or guide item"
        />
        <MetricCard
          label="Properties with Media"
          value={String(snapshot.propertyRows.length)}
          detail="Properties holding at least one media link"
        />
        <MetricCard
          label="Media Types"
          value={String(snapshot.byType.length)}
          detail="image / video / document"
        />
      </section>

      <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white p-6">
        <h2 className="font-serif text-2xl font-light">By Type</h2>

        <div className="mt-4 grid gap-4 md:grid-cols-3">
          {snapshot.byType.map((item) => (
            <div
              key={item.mediaType}
              className="rounded-sm border border-[var(--portal-border)] bg-[var(--portal-blue-pale)]/40 p-5"
            >
              <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
                {item.mediaType}
              </div>
              <div className="mt-3 font-serif text-2xl font-light">{item.total}</div>
              <div className="mt-2 text-xs font-light text-black/45">
                {item.linked} linked · {item.unlinked} unlinked
              </div>
              <div className="mt-1 text-xs font-light text-black/45">
                {item.missingAlt} missing alt text
              </div>
            </div>
          ))}
        </div>
      </section>

      <section className="mt-6 overflow-hidden rounded-sm border border-[var(--portal-border)] bg-white">
        <div className="flex items-center justify-between border-b border-[var(--portal-border)] px-6 py-5">
          <div>
            <h2 className="font-serif text-2xl font-light">Property Media Coverage</h2>
            <p className="mt-1 text-xs font-light text-black/40">
              Hero, gallery, video, and document presence per active property.
            </p>
          </div>
          <span className="text-xs font-light text-black/35">
            {snapshot.propertyRows.length} properties
          </span>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full min-w-[800px] border-collapse">
            <thead>
              <tr className="border-b border-[var(--portal-border)] bg-[var(--portal-blue-pale)]">
                <TableHeading>Property</TableHeading>
                <TableHeading>Hero</TableHeading>
                <TableHeading>Images</TableHeading>
                <TableHeading>Videos</TableHeading>
                <TableHeading>Documents</TableHeading>
                <TableHeading>Total</TableHeading>
              </tr>
            </thead>
            <tbody>
              {snapshot.propertyRows.length > 0 ? (
                snapshot.propertyRows.map((row) => (
                  <tr
                    key={row.propertyId}
                    className="border-b border-[var(--portal-border)] last:border-b-0 hover:bg-[var(--portal-blue-pale)]/40"
                  >
                    <td className="px-4 py-4 font-serif text-lg font-light">
                      {row.propertyName}
                    </td>
                    <td className="px-4 py-4 text-sm font-light">
                      {row.hasHero ? (
                        <span className="text-[var(--portal-success)]">✓</span>
                      ) : (
                        <span className="text-[var(--portal-archive)]">Missing</span>
                      )}
                    </td>
                    <td className="px-4 py-4 text-sm font-light">{row.imageCount}</td>
                    <td className="px-4 py-4 text-sm font-light">{row.videoCount}</td>
                    <td className="px-4 py-4 text-sm font-light">{row.documentCount}</td>
                    <td className="px-4 py-4 text-sm font-light">{row.totalCount}</td>
                  </tr>
                ))
              ) : (
                <tr>
                  <td
                    colSpan={6}
                    className="px-4 py-12 text-center text-sm font-light text-black/40"
                  >
                    No property media on file.
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
