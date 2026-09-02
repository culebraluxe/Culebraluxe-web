'use client'

import { useActionState, useEffect, useMemo, useRef, useState } from 'react'
import Link from 'next/link'
import { PageHeader } from '@/components/portal/page-header'
import { Panel } from '@/components/portal/panel'
import { CopyButton } from '@/components/portal/tech/copy-button'
import { STATUS_TONE } from '@/components/portal/marketing/status'
import {
  addSightingAction, confirmPlacementAction, logInquiryAction, publishListingsAction,
  renewPlacementAction, searchInquiryPeopleAction, withdrawPlacementAction,
  type MarketingWriteState,
} from '@/app/portal/marketing/actions'
import {
  CHANNEL_CATALOG,
  PREPARE_CHANNELS,
  REACHES_VIA_STELLAR_NOTE,
  MORE_CHANNELS,
  type SyndicationChannel,
} from '@/lib/syndication/channels'
import { isSourceStale } from '@/lib/syndication/hash'
import { isOffMarket, matchesNeedsFilter, placementNeedsMe, type NeedsFilter } from '@/lib/syndication/lifecycle'
import { buildPresenceReport, presenceReportText, type PresenceReport } from '@/lib/syndication/presence-report'
import { smsBlurb, waMeUrl, whatsappBlurb } from '@/lib/syndication/share'
import { takedownTextEn, takedownTextEs } from '@/lib/syndication/takedown'
import type { FacebookReadiness } from '@/lib/syndication/env'
import type {
  ListingPack, ListingSource, PlacementRow, SightingNetwork, SightingRow,
} from '@/lib/syndication/types'

const NETWORK_LABEL: Record<SightingNetwork, string> = {
  zillow: 'Zillow',
  realtor_com: 'Realtor.com',
  homes_com: 'Homes.com',
  other: 'Other',
}

function formatPrice(value: number | null) {
  if (value == null) return '—'
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', maximumFractionDigits: 0 }).format(value)
}

function isPack(value: ListingPack | Record<string, never>): value is ListingPack {
  return typeof value === 'object' && value !== null && 'titleEn' in value
}

function Banner({ state }: { state: MarketingWriteState }) {
  if (!state) return null
  return (
    <p className={`rounded-md px-3 py-2 text-sm font-light ${state.ok ? 'bg-emerald-50 text-emerald-800' : 'bg-rose-50 text-rose-800'}`}>
      {state.ok ? state.message ?? 'Done.' : state.error ?? 'Action failed.'}
    </p>
  )
}

export function SyndicationWorkbench({ sources, placements, sightings, facebook }: {
  sources: ListingSource[]; placements: PlacementRow[]; sightings: SightingRow[]; facebook?: FacebookReadiness
}) {
  const initial = sources.find((s) => s.isPublished)?.id ?? sources[0]?.id ?? ''
  const [sourceId, setSourceId] = useState(initial)
  const [selected, setSelected] = useState<SyndicationChannel[]>(['facebook_marketplace', 'stellar_mls'])
  const source = sources.find((s) => s.id === sourceId) ?? null
  const sourcePlacements = useMemo(() => placements.filter((p) => p.propertyId === sourceId), [placements, sourceId])
  const sourceSightings = useMemo(() => sightings.filter((s) => s.propertyId === sourceId), [sightings, sourceId])
  const presence = useMemo(
    () => (source ? buildPresenceReport(source, sourcePlacements, sourceSightings) : null),
    [source, sourcePlacements, sourceSightings],
  )
  const placementByChannel = useMemo(() => {
    const map = new Map<SyndicationChannel, PlacementRow>()
    for (const row of sourcePlacements) map.set(row.channel, row)
    return map
  }, [sourcePlacements])
  const [publishState, publishAction] = useActionState(publishListingsAction, null)
  const [confirmState, confirmAction] = useActionState(confirmPlacementAction, null)
  const [withdrawState, withdrawAction] = useActionState(withdrawPlacementAction, null)
  const [renewState, renewAction] = useActionState(renewPlacementAction, null)
  const [sightingState, sightingAction] = useActionState(addSightingAction, null)
  const [showMore, setShowMore] = useState(false)
  const [needsFilter, setNeedsFilter] = useState<NeedsFilter>('all')
  const needsCount = sourcePlacements.filter((row) => placementNeedsMe(source, row)).length
  const visiblePlacements = useMemo(
    () => sourcePlacements.filter((row) => matchesNeedsFilter(needsFilter, source, row)),
    [sourcePlacements, needsFilter, source],
  )
  const stellarRow = placementByChannel.get('stellar_mls') ?? null
  const facebookRow = placementByChannel.get('facebook_marketplace') ?? null
  const clasificadosRow = placementByChannel.get('clasificados') ?? null
  const launch = {
    site: !!source?.isPublished,
    stellar: !!stellarRow && (stellarRow.status === 'live' || !!stellarRow.externalId || !!stellarRow.externalUrl),
    facebook: !!facebookRow,
    clasificados: !!clasificadosRow,
    photos: (source?.imageCount ?? 0) >= 5 && !!source?.heroMediaId,
    office: true,
    portalSeen: sourceSightings.some((s) => s.network === 'zillow' || s.network === 'realtor_com'),
  }
  const takeDownRows = sourcePlacements.filter(
    (r) => (r.channel === 'facebook_marketplace' || r.channel === 'clasificados')
      && (r.status === 'live' || r.status === 'pending_manual'),
  )
  const fbOpen = takeDownRows.some((r) => r.channel === 'facebook_marketplace')
  const clasOpen = takeDownRows.some((r) => r.channel === 'clasificados')

  return (
    <div className="space-y-5">
      <style>{`
        #print-report { display: none; }
        @media print {
          #print-report { display: block; }
          body * { visibility: hidden; }
          #print-report, #print-report * { visibility: visible; }
          #print-report { position: absolute; left: 0; top: 0; width: 100%; padding: 0 1rem; }
        }
      `}</style>
      <PageHeader eyebrow="Marketing" title="Syndication" subtitle="Pick the root listing, choose channels, generate a pack per adapter. Manual sites wait for the live URL.">
        <Link href="/portal/marketing" className="text-[11px] font-light uppercase tracking-[0.16em] text-black/40">Dashboard</Link>
      </PageHeader>
      <Banner state={publishState} />
      <Banner state={confirmState} />
      <Banner state={withdrawState} />
      <Banner state={renewState} />
      <div className="grid gap-4 xl:grid-cols-[0.9fr_1.2fr]">
        <Panel eyebrow="Source" heading="Root listing" subtitle="Canonical property. Off-site ads point back here." flush>
          <div className="max-h-[34rem] overflow-y-auto">
            {sources.map((row) => {
              const active = row.id === sourceId
              const paths = placements.filter((p) => p.propertyId === row.id)
              return (
                <button key={row.id} type="button" onClick={() => setSourceId(row.id)} className={`flex w-full items-start gap-3 border-b border-[var(--portal-panel-border)] px-[var(--portal-panel-padding)] py-3 text-left last:border-b-0 ${active ? 'bg-white/80' : 'hover:bg-white/50'}`}>
                  <span className={`mt-1.5 h-2.5 w-2.5 shrink-0 rounded-full ${row.isPublished ? 'bg-emerald-400' : 'bg-black/20'}`} />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm text-[var(--portal-navy)]">{row.name}</span>
                    <span className="mt-0.5 block text-[11px] font-light text-black/45">{formatPrice(row.listPrice)}{row.city ? ` · ${row.city}` : ''}{row.isPublished ? ' · on site' : ' · not public'}{paths.length ? ` · ${paths.length} paths` : ''}</span>
                  </span>
                </button>
              )
            })}
          </div>
        </Panel>
        <div className="space-y-4">
          {source && isOffMarket(source) && sourcePlacements.length > 0 ? (
            <div className="rounded-md border border-rose-200 bg-rose-50 px-4 py-3">
              <p className="text-[11px] font-medium uppercase tracking-[0.16em] text-rose-700">Off the market</p>
              <p className="mt-1 text-sm font-light leading-5 text-rose-800">Take Facebook/Clasificados down and update Matrix — this listing is no longer being marketed.</p>
            </div>
          ) : null}
          {source && isOffMarket(source) && takeDownRows.length > 0 ? (
            <div className="rounded-md border border-rose-200 bg-rose-50/60 px-4 py-3">
              <p className="text-[11px] font-medium uppercase tracking-[0.16em] text-rose-700">Off-market takedown kit</p>
              <p className="mt-1 text-sm font-light leading-5 text-rose-800">Take the live paths down and update Matrix. This app does not delete third-party ads.</p>
              <ul className="mt-2 space-y-1 text-[13px] font-light text-black/70">
                <li>{fbOpen ? '☑' : '☐'} Facebook page / ad — take down</li>
                <li>{clasOpen ? '☑' : '☐'} Clasificados ad — take down</li>
                <li>☐ Stellar Matrix — mark withdrawn so feed portals update</li>
              </ul>
              <div className="mt-3 flex flex-wrap items-center gap-2">
                <CopyButton label="Copy takedown EN" text={takedownTextEn(source)} />
                <CopyButton label="Copy takedown ES" text={takedownTextEs(source)} />
                {takeDownRows.map((row) => (
                  <form key={row.id} action={withdrawAction}>
                    <input type="hidden" name="placementId" value={row.id} />
                    <button type="submit" className="text-[11px] font-light uppercase tracking-[0.14em] text-rose-600 hover:text-rose-800">Mark {CHANNEL_CATALOG[row.channel].shortLabel} withdrawn here</button>
                  </form>
                ))}
              </div>
            </div>
          ) : null}
          {facebook?.readyToPost ? (
            <div className="rounded-md border border-amber-300 bg-amber-50 px-4 py-3">
              <p className="text-[11px] font-medium uppercase tracking-[0.16em] text-amber-700">Live Graph POST is armed</p>
              <p className="mt-1 text-sm font-light leading-5 text-amber-800">SYNDICATION_LIVE=true and a Page token are set — preparing Facebook will POST to the Page feed. Set SYNDICATION_LIVE=false in Vercel to return to dry-run.</p>
            </div>
          ) : null}
          {source ? (
            <Panel eyebrow="Launch" heading={`Launch ${source.name}`} subtitle="Is this listing actually launched? A checklist, not another CRM.">
              <ul className="space-y-1.5">
                <LaunchRow done={launch.site} label="Published on culebraluxe.com" />
                <LaunchRow done={launch.stellar} label="Stellar pack prepared · MLS# confirmed" />
                <LaunchRow done={launch.facebook} label="Facebook pack / Page post" />
                <LaunchRow done={launch.clasificados} label="Clasificados (optional)" />
                <LaunchRow done={launch.photos} label="Photos ready with a hero" />
                <LaunchRow done={launch.office} label="Office name = CulebraLuxe" />
                <LaunchRow done={launch.portalSeen} label="Zillow / Realtor sighting pasted" hint="stays empty until you paste a public URL" />
              </ul>
              <InquiryLog propertyId={source.id} />
            </Panel>
          ) : null}
          <Panel eyebrow="Targets" heading="Channels" subtitle="Prepare only channels CulebraLuxe can reach. Zillow and Realtor.com are never upload targets here.">
            {source ? (
              <form action={publishAction} className="space-y-4">
                <input type="hidden" name="propertyId" value={source.id} />
                <ul className="grid gap-2 sm:grid-cols-2">
                  {PREPARE_CHANNELS.map((id) => {
                    const def = CHANNEL_CATALOG[id]
                    const placement = placementByChannel.get(id)
                    const tone = placement ? STATUS_TONE[placement.status] : null
                    const checked = selected.includes(id)
                    return (
                      <li key={id} className="rounded-[var(--portal-panel-radius)] border border-[var(--portal-panel-border)] bg-white/40 p-3">
                        <label className="flex items-start gap-2">
                          <input type="checkbox" name="channel" value={id} checked={checked} onChange={(event) => {
                            setSelected((current) => event.target.checked ? [...current, id] : current.filter((item) => item !== id))
                          }} className="mt-1" />
                          <span className="min-w-0 flex-1">
                            <span className="flex items-center justify-between gap-2">
                              <span className="text-sm text-[var(--portal-navy)]">{def.label}</span>
                              {tone ? (
                                <span className={`inline-flex items-center gap-1 text-[10px] uppercase tracking-[0.12em] ${tone.className}`}>
                                  <span className={`h-1.5 w-1.5 rounded-full ${tone.dot}`} />{tone.label}
                                </span>
                              ) : (
                                <span className="text-[10px] uppercase tracking-[0.12em] text-black/30">{def.readiness}</span>
                              )}
                            </span>
                            <span className="mt-1 block text-[11px] font-light leading-4 text-black/45">{def.notes}</span>
                          </span>
                        </label>
                      </li>
                    )
                  })}
                </ul>
                <div className="rounded-[var(--portal-panel-radius)] border border-[var(--portal-panel-border)] bg-white/30 p-3">
                  <p className="text-[10px] font-medium uppercase tracking-[0.16em] text-black/35">Reaches via Stellar</p>
                  <p className="mt-1 text-[11px] font-light leading-4 text-black/45">{REACHES_VIA_STELLAR_NOTE}</p>
                </div>
                <div className="rounded-[var(--portal-panel-radius)] border border-[var(--portal-panel-border)] bg-white/30 p-3">
                  <button type="button" onClick={() => setShowMore((value) => !value)} className="flex w-full items-center justify-between gap-2 text-left">
                    <span className="text-[10px] font-medium uppercase tracking-[0.16em] text-black/35">More (not this quarter)</span>
                    <span className="text-[10px] uppercase tracking-[0.12em] text-black/30">{showMore ? 'Less' : 'Show'}</span>
                  </button>
                  {showMore ? (
                    <ul className="mt-2 space-y-1.5">
                      {MORE_CHANNELS.map((id) => {
                        const def = CHANNEL_CATALOG[id]
                        const placement = placementByChannel.get(id)
                        const tone = placement ? STATUS_TONE[placement.status] : null
                        return (
                          <li key={id} className="text-[11px] font-light leading-4 text-black/45">
                            <span className="text-black/60">{def.label}</span> · {def.mode} / {def.readiness}{tone ? ` · ${tone.label}` : ''}
                          </li>
                        )
                      })}
                    </ul>
                  ) : null}
                </div>
                <div className="flex flex-wrap items-center gap-3">
                  <button type="submit" disabled={!source || selected.length === 0} className="inline-flex min-h-9 items-center rounded-md border border-[var(--portal-gold)]/50 bg-[var(--portal-navy)] px-3 text-[11px] font-light uppercase tracking-[0.16em] text-white disabled:opacity-40">Prepare selected</button>
                  <p className="text-[11px] font-light text-black/40">Does not upload to Zillow or Realtor.com.</p>
                </div>
              </form>
            ) : <p className="text-sm font-light text-black/45">Select a listing first.</p>}
          </Panel>
          {source ? (
            <Panel eyebrow="Constellation" heading="Observed on portals" subtitle="Where this listing actually showed up. Paste a public URL — this never publishes to the network.">
              {sourceSightings.length > 0 ? (
                <ul className="space-y-1.5">
                  {sourceSightings.map((sighting) => (
                    <li key={sighting.id} className="flex items-start justify-between gap-3 text-[12px] font-light text-black/60">
                      <span>
                        Seen on <span className="text-black/80">{NETWORK_LABEL[sighting.network] ?? sighting.network}</span>
                        {sighting.notes ? <span className="text-black/45"> · {sighting.notes}</span> : null}
                      </span>
                      {sighting.url ? (
                        <a href={sighting.url} target="_blank" rel="noreferrer" className="shrink-0 text-[11px] uppercase tracking-[0.12em] text-[var(--portal-navy)] underline">link</a>
                      ) : null}
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="text-sm font-light text-black/45">No sightings yet. Once it appears on Zillow or Realtor.com, paste the public URL here to pin it.</p>
              )}
              <Banner state={sightingState} />
              <form action={sightingAction} className="mt-3 space-y-2">
                <input type="hidden" name="propertyId" value={source.id} />
                <div className="flex flex-wrap items-center gap-2">
                  <select name="network" className="min-h-9 rounded-md border border-[var(--portal-panel-border)] bg-white/80 px-2 text-sm font-light">
                    {(Object.keys(NETWORK_LABEL) as SightingNetwork[]).map((key) => (
                      <option key={key} value={key}>{NETWORK_LABEL[key]}</option>
                    ))}
                  </select>
                  <input name="url" required placeholder="https:// observed public URL" className="min-h-9 min-w-[14rem] flex-1 rounded-md border border-[var(--portal-panel-border)] bg-white/80 px-3 text-sm font-light" />
                  <button type="submit" className="inline-flex min-h-9 items-center rounded-md bg-[var(--portal-navy)] px-3 text-[11px] font-light uppercase tracking-[0.16em] text-white">Note it</button>
                </div>
              </form>
            </Panel>
          ) : null}
          {sourcePlacements.length > 0 ? (
            <Panel eyebrow="Round trip" heading="Packs & confirmation">
              <div className="mb-3 flex flex-wrap items-center gap-1.5">
                {(([['all', 'All'], ['needs_me', needsCount ? `Needs me (${needsCount})` : 'Needs me'], ['live', 'Live'], ['expired', 'Expired']]) as Array<[NeedsFilter, string]>).map(([key, label]) => (
                  <button key={key} type="button" onClick={() => setNeedsFilter(key)} className={`min-h-8 rounded-md border px-2.5 text-[11px] font-light uppercase tracking-[0.12em] ${needsFilter === key ? 'border-[var(--portal-navy)] bg-[var(--portal-navy)] text-white' : 'border-[var(--portal-panel-border)] bg-white/40 text-black/45'}`}>{label}</button>
                ))}
              </div>
              <div className="space-y-4">
                {visiblePlacements.length === 0 ? (
                  <p className="text-sm font-light text-black/45">Nothing needs you under this filter.</p>
                ) : visiblePlacements.map((row) => {
                  const pack = isPack(row.pack) ? row.pack : null
                  const tone = STATUS_TONE[row.status]
                  const def = CHANNEL_CATALOG[row.channel]
                  const transportJson = pack?.transport ? JSON.stringify(pack.transport, null, 2) : null
                  return (
                    <article key={row.id} className="rounded-[var(--portal-panel-radius)] border border-[var(--portal-panel-border)] bg-white/50 p-4">
                      <div className="flex flex-wrap items-start justify-between gap-2">
                        <div>
                          <h3 className="font-serif text-xl font-light text-[var(--portal-navy)]">{def.label}</h3>
                          <p className={`mt-1 text-[11px] uppercase tracking-[0.14em] ${tone.className}`}>{tone.label}{row.lastAttemptAt ? ` · ${row.lastAttemptAt}` : ''}</p>
                        </div>
                        {row.externalUrl ? <a href={row.externalUrl} target="_blank" rel="noreferrer" className="text-[11px] uppercase tracking-[0.14em] text-[var(--portal-navy)] underline">Open live ad</a> : null}
                      </div>
                      {row.channel !== 'culebraluxe' && source && row.sourceHash && isSourceStale(source, row.sourceHash) ? (
                        <p className="mt-3 rounded-md border border-amber-300/70 bg-amber-50 px-3 py-2 text-[12px] font-light text-amber-800">Price/facts changed — regenerate this pack.</p>
                      ) : null}
                      {pack ? (
                        <div className="mt-3 space-y-3">
                          <p className="text-[12px] font-light leading-5 text-black/50">{pack.instructions}</p>
                          <div className="grid gap-3 md:grid-cols-2">
                            <PackField label="Title ES" value={pack.titleEs} />
                            <PackField label="Title EN" value={pack.titleEn} />
                            <PackField label="Body ES" value={pack.bodyEs} />
                            <PackField label="Body EN" value={pack.bodyEn} />
                          </div>
                          {transportJson ? <PackField label="API transport" value={transportJson} /> : null}
                          <div className="flex flex-wrap gap-2">
                            <CopyButton label="Copy ES" text={`${pack.titleEs}\n\n${pack.bodyEs}`} />
                            <CopyButton label="Copy EN" text={`${pack.titleEn}\n\n${pack.bodyEn}`} />
                            {transportJson ? <CopyButton label="Copy payload" text={transportJson} /> : null}
                            {pack.pasteTargetUrl ? <a href={pack.pasteTargetUrl} target="_blank" rel="noreferrer" className="inline-flex min-h-9 items-center rounded-md border border-[var(--portal-border)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy)]">Open target</a> : null}
                          </div>
                        </div>
                      ) : null}
                      {row.lastError ? <p className="mt-3 text-sm font-light text-rose-700">{row.lastError}</p> : null}
                      {row.status !== 'withdrawn' ? (
                        <form action={confirmAction} className="mt-4 flex flex-wrap gap-2">
                          <input type="hidden" name="placementId" value={row.id} />
                          <input name="externalUrl" defaultValue={row.externalUrl ?? ''} placeholder="https:// live ad URL" className="min-h-9 min-w-[16rem] flex-1 rounded-md border border-[var(--portal-panel-border)] bg-white/80 px-3 text-sm font-light" />
                          <button type="submit" className="inline-flex min-h-9 items-center rounded-md bg-emerald-700 px-3 text-[11px] font-light uppercase tracking-[0.16em] text-white">{row.status === 'live' ? 'Refresh URL' : 'Confirm live'}</button>
                        </form>
                      ) : null}
                      {row.status !== 'withdrawn' ? (
                        <form action={withdrawAction} className="mt-2">
                          <input type="hidden" name="placementId" value={row.id} />
                          <button type="submit" className="text-[11px] font-light uppercase tracking-[0.14em] text-black/35 hover:text-rose-700">Withdraw path</button>
                        </form>
                      ) : null}
                      {row.status === 'expired' && (row.channel === 'facebook_marketplace' || row.channel === 'clasificados') ? (
                        <form action={renewAction} className="mt-2">
                          <input type="hidden" name="placementId" value={row.id} />
                          <button type="submit" className="text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-gold)] hover:text-amber-600">Renew expired pack</button>
                        </form>
                      ) : null}
                    </article>
                  )
                })}
              </div>
            </Panel>
          ) : null}
        </div>
      </div>
      {source && presence ? (
        <div className="grid gap-4 xl:grid-cols-2">
          <SellerReportPanel report={presence} source={source} />
          <SharePanel source={source} />
        </div>
      ) : null}
    </div>
  )
}

function SellerReportPanel({ report, source }: { report: PresenceReport; source: ListingSource }) {
  const copyText = presenceReportText(report)
  const url = source.publicUrl ?? ''
  const qrSrc = url ? `/portal/marketing/qr?url=${encodeURIComponent(url)}` : ''
  const rows: Array<[string, string]> = [
    ['Listing', `${report.propertyName} — ${report.priceLabel}`],
    ['Location', report.city],
    ['Facts', report.factsLine],
  ]
  if (report.daysOnMarket != null) rows.push(['Days on market', `${report.daysOnMarket}`])
  rows.push(
    ['On culebraluxe.com', report.sitePublished ? 'Live' : 'Not published'],
    ['Stellar / MLS', report.stellarMls ? `${report.stellarStatus} · #${report.stellarMls}` : report.stellarStatus],
    ['Facebook', report.facebookUrl ?? report.facebookStatus],
    ['Clasificados', report.clasificadosUrl ?? report.clasificadosStatus],
  )
  return (
    <>
      <Panel eyebrow="Seller report" heading="Presence one-pager" subtitle="Print or copy this to email the seller. It lists confirmed or observed URLs, never a Zillow upload.">
        <dl className="space-y-1.5 text-[13px] font-light">
          {rows.map(([key, value]) => (
            <div key={key} className="flex flex-wrap justify-between gap-3">
              <dt className="text-black/45">{key}</dt>
              <dd className="max-w-[16rem] text-right text-black/80">{value}</dd>
            </div>
          ))}
          {report.sightings.length === 0 ? (
            <div className="flex flex-wrap justify-between gap-3">
              <dt className="text-black/45">Zillow / Realtor.com</dt>
              <dd className="text-black/80">Not observed yet</dd>
            </div>
          ) : report.sightings.map((sighting) => (
            <div key={sighting.network} className="flex flex-wrap justify-between gap-3">
              <dt className="text-black/45">Seen on {sighting.network}</dt>
              <dd className="max-w-[16rem] text-right text-black/80">{sighting.url ?? sighting.notes ?? 'Observed'}</dd>
            </div>
          ))}
        </dl>
        <p className="mt-3 text-[11px] font-light leading-4 text-black/40">{report.disclaimer}</p>
        <p className="mt-1 text-[11px] font-light text-black/35">Generated {report.generatedAt} (PR time)</p>
        <div className="mt-3 flex flex-wrap gap-2">
          <CopyButton label="Copy report" text={copyText} />
          <button type="button" onClick={() => window.print()} className="inline-flex min-h-9 items-center rounded-md border border-[var(--portal-border)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy)]">Print report</button>
        </div>
      </Panel>
      <div id="print-report">
        <h1 style={{ fontSize: '1.5rem', marginBottom: '0.25rem' }}>{report.propertyName}</h1>
        <p style={{ margin: '0 0 0.25rem' }}>{report.priceLabel} · {report.city}</p>
        <p style={{ margin: '0 0 0.5rem' }}>{report.factsLine}</p>
        {report.daysOnMarket != null ? <p style={{ margin: '0 0 0.5rem' }}>Days on market: {report.daysOnMarket}</p> : null}
        <table style={{ borderCollapse: 'collapse', width: '100%', fontSize: '13px' }}>
          <tbody>
            {rows.map(([key, value]) => (
              <tr key={key}>
                <td style={{ padding: '2px 8px 2px 0', color: '#333', whiteSpace: 'nowrap', verticalAlign: 'top' }}>{key}</td>
                <td style={{ padding: '2px 0', color: '#000' }}>{value}</td>
              </tr>
            ))}
            {report.sightings.length === 0 ? (
              <tr><td style={{ padding: '2px 8px 2px 0', color: '#333', whiteSpace: 'nowrap', verticalAlign: 'top' }}>Zillow / Realtor.com</td><td>Not observed yet</td></tr>
            ) : report.sightings.map((sighting) => (
              <tr key={sighting.network}>
                <td style={{ padding: '2px 8px 2px 0', color: '#333', whiteSpace: 'nowrap', verticalAlign: 'top' }}>Seen on {sighting.network}</td>
                <td>{sighting.url ?? sighting.notes ?? 'Observed'}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <p style={{ marginTop: '0.75rem', fontSize: '11px', color: '#666' }}>{report.disclaimer}</p>
        {qrSrc ? (
          <div style={{ marginTop: '1rem' }}>
            {/* eslint-disable-next-line @next/next/no-img-element */}
            <img src={qrSrc} alt={`QR to ${url}`} width={160} height={160} style={{ width: '160px', height: '160px' }} />
            <p style={{ fontSize: '11px', color: '#333', maxWidth: '160px' }}>{url}</p>
          </div>
        ) : null}
        <p style={{ marginTop: '0.5rem', fontSize: '10px', color: '#888' }}>Generated {report.generatedAt} (Puerto Rico time)</p>
      </div>
    </>
  )
}

function SharePanel({ source }: { source: ListingSource }) {
  const url = source.publicUrl ?? ''
  const qrSrc = url ? `/portal/marketing/qr?url=${encodeURIComponent(url)}` : ''
  return (
    <Panel eyebrow="Share" heading="Send to a buyer or seller" subtitle="One-tap WhatsApp/SMS blurbs and a QR to the public listing.">
      <div className="flex items-start gap-4">
        <div className="flex flex-1 flex-col gap-2">
          <CopyButton label="WhatsApp EN" text={whatsappBlurb(source, 'en')} />
          <CopyButton label="WhatsApp ES" text={whatsappBlurb(source, 'es')} />
          <CopyButton label="SMS EN" text={smsBlurb(source, 'en')} />
          <CopyButton label="SMS ES" text={smsBlurb(source, 'es')} />
          {url ? (
            <a href={waMeUrl(source, 'en')} target="_blank" rel="noreferrer" className="inline-flex min-h-9 items-center rounded-md border border-[var(--portal-border)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy)]">Open in WhatsApp</a>
          ) : null}
        </div>
        {qrSrc ? (
          <div className="shrink-0 rounded-md border border-[var(--portal-panel-border)] bg-white p-2">
            {/* eslint-disable-next-line @next/next/no-img-element */}
            <img src={qrSrc} alt={`QR to ${url}`} width={160} height={160} className="h-40 w-40" />
            <p className="mt-1 max-w-[10rem] text-center text-[10px] font-light text-black/40">Scan to open the listing</p>
          </div>
        ) : null}
      </div>
      <p className="mt-3 text-[11px] font-light text-black/40">Every blurb and the QR point to the public listing URL only.</p>
    </Panel>
  )
}

type PersonOption = { id: string; displayName: string; email: string | null; phone: string | null }
const INQUIRY_SOURCE_OPTIONS: Array<[string, string]> = [
  ['phone', 'Phone'],
  ['whatsapp', 'WhatsApp'],
  ['email', 'Email'],
  ['walkin', 'Walk-in'],
]

function InquiryLog({ propertyId }: { propertyId: string }) {
  const [query, setQuery] = useState('')
  const [options, setOptions] = useState<PersonOption[]>([])
  const [chosen, setChosen] = useState<PersonOption | null>(null)
  const [state, formAction] = useActionState(logInquiryAction, null)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  useEffect(() => () => { if (timer.current) clearTimeout(timer.current) }, [])
  async function onType(value: string) {
    setQuery(value)
    if (timer.current) clearTimeout(timer.current)
    timer.current = setTimeout(() => {
      if (!value.trim()) { setOptions([]); return }
      void searchInquiryPeopleAction(value.trim())
        .then((res) => setOptions(res as PersonOption[]))
        .catch(() => setOptions([]))
    }, 300)
  }
  return (
    <div className="mt-4 border-t border-[var(--portal-panel-border)] pt-3">
      <p className="text-[10px] font-medium uppercase tracking-[0.16em] text-black/35">Log an inquiry</p>
      {!chosen ? (
        <>
          <input
            value={query}
            onChange={(event) => onType(event.target.value)}
            placeholder="Search an existing person…"
            className="mt-2 min-h-9 w-full rounded-md border border-[var(--portal-panel-border)] bg-white/80 px-3 text-sm font-light"
          />
          {options.length > 0 ? (
            <ul className="mt-1.5 space-y-1">
              {options.map((person) => (
                <li key={person.id}>
                  <button
                    type="button"
                    onClick={() => { setChosen(person); setOptions([]) }}
                    className="w-full rounded-md border border-[var(--portal-panel-border)] bg-white/60 px-3 py-1.5 text-left text-[13px] font-light text-black/70 hover:bg-white"
                  >
                    {person.displayName}{person.phone ? ` · ${person.phone}` : ''}{person.email ? ` · ${person.email}` : ''}
                  </button>
                </li>
              ))}
            </ul>
          ) : null}
        </>
      ) : (
        <div className="mt-2 flex flex-wrap items-center justify-between gap-2 rounded-md border border-[var(--portal-panel-border)] bg-white/60 px-3 py-2">
          <span className="text-[13px] font-light text-black/80">{chosen.displayName}</span>
          <span className="flex items-center gap-2">
            <Link href={`/portal/clients/${chosen.id}`} className="text-[11px] uppercase tracking-[0.12em] text-[var(--portal-navy)] underline">Open client</Link>
            <button type="button" onClick={() => setChosen(null)} className="text-[11px] font-light uppercase tracking-[0.12em] text-black/40 hover:text-rose-700">Change</button>
          </span>
        </div>
      )}
      <form action={formAction} className="mt-2 space-y-2">
        <input type="hidden" name="propertyId" value={propertyId} />
        <input type="hidden" name="personId" value={chosen?.id ?? ''} />
        <div className="flex flex-wrap items-center gap-2">
          <select name="source" className="min-h-9 rounded-md border border-[var(--portal-panel-border)] bg-white/80 px-2 text-sm font-light">
            {INQUIRY_SOURCE_OPTIONS.map(([value, label]) => <option key={value} value={value}>{label}</option>)}
          </select>
          <input name="notes" placeholder="Inquiry note" className="min-h-9 min-w-[12rem] flex-1 rounded-md border border-[var(--portal-panel-border)] bg-white/80 px-3 text-sm font-light" />
          <button type="submit" disabled={!chosen} className="inline-flex min-h-9 items-center rounded-md bg-[var(--portal-navy)] px-3 text-[11px] font-light uppercase tracking-[0.16em] text-white disabled:opacity-40">Log inquiry</button>
        </div>
        <Banner state={state} />
      </form>
    </div>
  )
}

function LaunchRow({ done, label, hint }: { done: boolean; label: string; hint?: string }) {
  return (
    <li className="flex items-start gap-2">
      <span className={`mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full text-[9px] leading-none ${done ? 'bg-emerald-500 text-white' : 'border border-black/25 text-transparent'}`}>✓</span>
      <span className="text-[13px] font-light leading-5 text-black/60">{label}{hint ? <span className="text-black/35"> — {hint}</span> : null}</span>
    </li>
  )
}

function PackField({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="mb-1 flex items-center justify-between">
        <p className="text-[10px] font-medium uppercase tracking-[0.16em] text-black/35">{label}</p>
        <CopyButton label="Copy" text={value} />
      </div>
      <pre className="max-h-40 overflow-auto whitespace-pre-wrap rounded-md bg-[var(--portal-navy)]/5 p-3 text-[12px] font-light leading-5 text-black/70">{value}</pre>
    </div>
  )
}
