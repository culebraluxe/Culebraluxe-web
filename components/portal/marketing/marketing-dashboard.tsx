import Link from 'next/link'
import { PageHeader } from '@/components/portal/page-header'
import { Panel } from '@/components/portal/panel'
import { EVENT_LABEL, STATUS_TONE } from '@/components/portal/marketing/status'
import type { MarketingDashboardSnapshot } from '@/db/syndication'
import { CHANNEL_CATALOG, SYNDICATION_CHANNELS } from '@/lib/syndication/channels'
import type { PlacementRow, SightingNetwork, SightingRow, SyndicationEventRow } from '@/lib/syndication/types'
import type { FacebookReadiness } from '@/lib/syndication/env'

type EventRow = SyndicationEventRow & { channel: PlacementRow['channel']; propertyName: string }

function Metric({ label, value, hint, tone = 'neutral' }: {
  label: string; value: string | number; hint: string; tone?: 'neutral' | 'good' | 'warn' | 'bad'
}) {
  const valueClass = tone === 'good' ? 'text-emerald-200' : tone === 'warn' ? 'text-amber-200' : tone === 'bad' ? 'text-rose-200' : 'text-white'
  return (
    <div className="rounded-[var(--portal-panel-radius)] border border-white/10 bg-gradient-to-b from-white/[0.09] to-white/[0.03] p-4">
      <p className="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold)]">{label}</p>
      <p className={`mt-2 font-serif text-3xl font-light ${valueClass}`}>{value}</p>
      <p className="mt-1 text-[11px] font-light text-white/50">{hint}</p>
    </div>
  )
}

const EVENT_CHANNEL = { zillow: 'Zillow', realtor_com: 'Realtor.com', homes_com: 'Homes.com', other: 'Other' }

export function MarketingDashboard({ snapshot, placements, events, sightings, facebook }: {
  snapshot: MarketingDashboardSnapshot; placements: PlacementRow[]; events: EventRow[]
  sightings: SightingRow[]; facebook?: FacebookReadiness
}) {
  const byProperty = new Map<string, PlacementRow[]>()
  for (const row of placements) {
    const list = byProperty.get(row.propertyId) ?? []
    list.push(row)
    byProperty.set(row.propertyId, list)
  }
  const sightingsByProperty = new Map<string, SightingRow[]>()
  for (const s of sightings) {
    const list = sightingsByProperty.get(s.propertyId) ?? []
    list.push(s)
    sightingsByProperty.set(s.propertyId, list)
  }
  const constellations = [...byProperty.entries()].map(([propertyId, rows]) => ({
    propertyId, name: rows[0]?.propertyName ?? 'Listing', rows,
    sightings: sightingsByProperty.get(propertyId) ?? [],
    live: rows.filter((r) => r.status === 'live').length,
    pending: rows.filter((r) => r.status === 'pending_manual').length,
  })).slice(0, 8)

  return (
    <div className="space-y-5">
      <PageHeader eyebrow="Marketing" title="Presence" subtitle="One canonical listing, many outbound paths — same 1→N shape as a person and their identities. HubSpot stays a sibling system.">
        <Link href="/portal/marketing/syndication" className="inline-flex min-h-9 items-center rounded-md border border-[var(--portal-gold)]/50 bg-[var(--portal-navy)] px-3 text-[11px] font-light uppercase tracking-[0.16em] text-white">Open syndication</Link>
      </PageHeader>
      {facebook ? (
        <p className="text-[11px] font-light uppercase tracking-[0.14em] text-black/40">
          Meta {facebook.readyToPost ? 'live-ready' : 'dry-run'}
          {facebook.requiredMissing.length > 0 ? ` · missing ${facebook.requiredMissing.join(', ')}` : ''}
          {' · '}SYNDICATION_LIVE {facebook.liveEnabled ? 'true' : 'false'}
        </p>
      ) : null}
      <div className="rounded-[var(--portal-panel-radius)] border border-[var(--portal-gold)]/25 bg-[var(--portal-navy-deep)] p-4 text-white sm:p-5">
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-6">
          <Metric label="Inventory" value={snapshot.inventory} hint="Active properties" />
          <Metric label="On site" value={snapshot.publishedOnSite} hint="property.is_published" tone={snapshot.publishedOnSite > 0 ? 'good' : 'neutral'} />
          <Metric label="Live off-site" value={snapshot.livePlacements} hint="Confirmed placements" tone={snapshot.livePlacements > 0 ? 'good' : 'neutral'} />
          <Metric label="Awaiting confirm" value={snapshot.pendingManual} hint="Pack pasted, URL not back" tone={snapshot.pendingManual > 0 ? 'warn' : 'neutral'} />
          <Metric label="Expired" value={snapshot.expired} hint="TTL lapsed" tone={snapshot.expired > 0 ? 'warn' : 'neutral'} />
          <Metric label="Failed" value={snapshot.failed} hint="Adapter or confirm error" tone={snapshot.failed > 0 ? 'bad' : 'neutral'} />
        </div>
      </div>
      <div className="grid gap-4 lg:grid-cols-[1.2fr_0.8fr]">
        <Panel eyebrow="Channels" heading="Adapter readiness" subtitle="Pack channels generate paste text. MLS and HubSpot stay stubs." lifted>
          <ul className="grid gap-2 sm:grid-cols-2">
            {SYNDICATION_CHANNELS.map((id) => {
              const def = CHANNEL_CATALOG[id]
              const live = placements.filter((p) => p.channel === id && p.status === 'live').length
              const pending = placements.filter((p) => p.channel === id && p.status === 'pending_manual').length
              return (
                <li key={id} className="rounded-[var(--portal-panel-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-3 py-3">
                  <div className="flex items-start justify-between gap-2">
                    <div>
                      <p className="text-sm font-medium text-[var(--portal-navy)]">{def.label}</p>
                      <p className="mt-0.5 text-[11px] font-light uppercase tracking-[0.12em] text-black/40">{def.mode} · {def.readiness}</p>
                    </div>
                    <p className="text-[11px] font-light text-black/45">{live} live{pending ? ` · ${pending} open` : ''}</p>
                  </div>
                  <p className="mt-2 text-[12px] font-light leading-5 text-black/50">{def.notes}</p>
                </li>
              )
            })}
          </ul>
        </Panel>
        <Panel variant="feature" eyebrow="Sibling system" heading="HubSpot" subtitle="Account exists. It does not sit inside this ledger.">
          <p className="text-sm font-light leading-6 text-white/70">Marketing owns listing presence. HubSpot owns CRM campaigns and contacts. Side by side — no dual-write in this first stab.</p>
          <p className="mt-4 text-[11px] font-light uppercase tracking-[0.16em] text-[var(--portal-gold)]">Adapter status · stub</p>
        </Panel>
      </div>
      <div className="grid gap-4 lg:grid-cols-2">
        <Panel eyebrow="1 → N" heading="Listing constellations" subtitle="Each property is the root. Each channel is one outbound path." flush>
          {constellations.length === 0 ? (
            <p className="px-[var(--portal-panel-padding)] pb-5 text-sm font-light text-black/45">No placements yet. Open Syndication and generate packs.</p>
          ) : (
            <ul>
              {constellations.map((item) => (
                <li key={item.propertyId} className="border-b border-[var(--portal-panel-border)] px-[var(--portal-panel-padding)] py-3 last:border-b-0">
                  <div className="flex items-baseline justify-between gap-3">
                    <p className="truncate text-sm text-[var(--portal-navy)]">{item.name}</p>
                    <p className="shrink-0 text-[11px] font-light text-black/40">{item.live} live · {item.pending} pending</p>
                  </div>
                  <div className="mt-2 flex flex-wrap gap-1.5">
                    {item.rows.map((row) => {
                      const tone = STATUS_TONE[row.status]
                      return (
                        <span key={row.id} className="inline-flex items-center gap-1.5 rounded-full border border-[var(--portal-panel-border)] bg-white/60 px-2 py-0.5 text-[10px] uppercase tracking-[0.12em] text-black/55">
                          <span className={`h-1.5 w-1.5 rounded-full ${tone.dot}`} />
                          {CHANNEL_CATALOG[row.channel].shortLabel}
                        </span>
                      )
                    })}
                  </div>
                  {item.sightings.length > 0 ? (
                    <div className="mt-1.5 flex flex-wrap gap-1.5">
                      {item.sightings.map((s) => {
                        const label = `Seen on ${EVENT_CHANNEL[s.network as SightingNetwork] ?? s.network}`
                        return s.url ? (
                          <a key={s.id} href={s.url} target="_blank" rel="noreferrer" className="inline-flex items-center gap-1 rounded-full border border-[var(--portal-gold)]/45 bg-white/70 px-2 py-0.5 text-[10px] uppercase tracking-[0.12em] text-[var(--portal-navy)] hover:bg-[var(--portal-gold)]/15">{label}</a>
                        ) : (
                          <span key={s.id} className="inline-flex items-center gap-1 rounded-full border border-[var(--portal-gold)]/35 bg-white/50 px-2 py-0.5 text-[10px] uppercase tracking-[0.12em] text-black/45">{label}</span>
                        )
                      })}
                    </div>
                  ) : null}
                </li>
              ))}
            </ul>
          )}
        </Panel>
        <Panel eyebrow="Round trip" heading="Recent events" flush>
          {events.length === 0 ? (
            <p className="px-[var(--portal-panel-padding)] pb-5 text-sm font-light text-black/45">Events appear when a pack is generated or a placement is confirmed.</p>
          ) : (
            <ul>
              {events.map((event) => (
                <li key={event.id} className="border-b border-[var(--portal-panel-border)] px-[var(--portal-panel-padding)] py-3 last:border-b-0">
                  <p className="text-sm text-[var(--portal-navy)]">{EVENT_LABEL[event.eventType] ?? event.eventType}<span className="text-black/35"> · {CHANNEL_CATALOG[event.channel].shortLabel}</span></p>
                  <p className="mt-0.5 text-[12px] font-light text-black/45">{event.propertyName}<span className="text-black/30"> · {event.createdAt}</span></p>
                </li>
              ))}
            </ul>
          )}
        </Panel>
      </div>
    </div>
  )
}
