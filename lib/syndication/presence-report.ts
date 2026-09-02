import type { ListingSource, PlacementRow, SightingRow } from './types'

// ---------------------------------------------------------------------------
// Slice C — Seller presence one-pager. Pure: build a display model the UI can
// print/copy from the same stored data (site + Stellar + Facebook + sightings).
// Disclaimer language keeps it honest: portals update from Stellar; this lists
// confirmed or observed URLs, never "published to Zillow".
// ---------------------------------------------------------------------------

export type PresenceNetworkLine = { network: string; url: string | null; notes: string | null }

export type PresenceReport = {
  propertyName: string
  priceLabel: string
  city: string
  factsLine: string
  siteUrl: string | null
  sitePublished: boolean
  stellarStatus: string
  stellarMls: string | null
  facebookStatus: string
  facebookUrl: string | null
  clasificadosStatus: string
  clasificadosUrl: string | null
  sightings: PresenceNetworkLine[]
  disclaimer: string
  generatedAt: string
}

const DISCLAIMER =
  'Portals (Zillow, Realtor.com, Homes.com) update from the Stellar MLS feed. ' +
  'This lists confirmed or observed URLs — it is not an upload log. Nothing below was "published to Zillow" by this app.'

export function priceLabel(value: number | null): string {
  if (value == null || !Number.isFinite(value)) return 'Price on request'
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: 0,
  }).format(value)
}

export function factsLine(source: ListingSource): string {
  const parts: string[] = []
  if (source.bedrooms != null) parts.push(`${source.bedrooms} bed`)
  if (source.bathrooms != null) parts.push(`${source.bathrooms} bath`)
  if (source.squareFeet != null) parts.push(`${source.squareFeet.toLocaleString('en-US')} sqft`)
  return parts.join(' · ')
}

function statusByChannel(channel: string, rows: PlacementRow[]): PlacementRow | null {
  return rows.find((r) => r.channel === channel) ?? null
}

export function buildPresenceReport(
  source: ListingSource,
  placements: PlacementRow[],
  sightings: SightingRow[],
): PresenceReport {
  const rows = placements.filter((p) => p.propertyId === source.id)
  const siteUrl = source.publicUrl

  const stellar = statusByChannel('stellar_mls', rows)
  const stellarConfirmed = !!stellar && (stellar.status === 'live' || !!stellar.externalUrl || !!stellar.externalId)

  const facebook = statusByChannel('facebook_marketplace', rows)
  const facebookConfirmed = !!facebook && (facebook.status === 'live' || !!facebook.externalUrl)

  const clasificados = statusByChannel('clasificados', rows)
  const clasificadosConfirmed = !!clasificados && (clasificados.status === 'live' || !!clasificados.externalUrl)

  return {
    propertyName: source.name,
    priceLabel: priceLabel(source.listPrice),
    city: source.city ?? 'Culebra, Puerto Rico',
    factsLine: factsLine(source),
    siteUrl,
    sitePublished: source.isPublished,
    stellarStatus: stellarConfirmed
      ? 'In Stellar — portals follow the feed'
      : stellar
        ? 'Pack prepared — enter in Matrix, then paste the MLS# to confirm'
        : 'Not in Matrix yet',
    stellarMls: stellar?.externalId ?? null,
    facebookStatus: facebookConfirmed
      ? facebook?.status === 'live' && facebook?.externalUrl
        ? 'Page / Marketplace URL confirmed'
        : 'Pack ready — paste the live post URL to confirm'
      : facebook
        ? 'Pack ready (not yet confirmed)'
        : 'Not prepared yet',
    facebookUrl: facebook?.externalUrl ?? null,
    clasificadosStatus: clasificadosConfirmed
      ? 'Clasificados live URL confirmed'
      : clasificados
        ? 'Clasificados pack ready — paste the live ad URL'
        : 'Not prepared (optional)',
    clasificadosUrl: clasificados?.externalUrl ?? null,
    sightings: sightings
      .filter((s) => s.propertyId === source.id)
      .map((s) => ({ network: s.network, url: s.url, notes: s.notes })),
    disclaimer: DISCLAIMER,
    generatedAt: new Intl.DateTimeFormat('en-US', {
      timeZone: 'America/Puerto_Rico',
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(new Date()),
  }
}

/** Plain-text rendering for Copy and for window.print() bodies. */
export function presenceReportText(report: PresenceReport): string {
  const lines = [
    `${report.propertyName} — ${report.priceLabel}`,
    report.city,
    report.factsLine,
    '',
    `Site: ${report.sitePublished && report.siteUrl ? 'Live on culebraluxe.com' : 'Not published on site'}`,
    report.siteUrl ? `  ${report.siteUrl}` : '',
    `Stellar: ${report.stellarStatus}`,
    report.stellarMls ? `  MLS# ${report.stellarMls}` : '',
    report.facebookUrl ? `Facebook: ${report.facebookUrl}` : `Facebook: ${report.facebookStatus}`,
    report.clasificadosUrl ? `Clasificados: ${report.clasificadosUrl}` : `Clasificados: ${report.clasificadosStatus}`,
  ]
  if (report.sightings.length === 0) {
    lines.push('Zillow / Realtor.com: Not observed yet.')
  } else {
    lines.push('Observed on portals:')
    for (const s of report.sightings) {
      lines.push(`  ${s.network}${s.url ? ` · ${s.url}` : ''}${s.notes ? ` · ${s.notes}` : ''}`)
    }
  }
  lines.push('', report.disclaimer, `Generated ${report.generatedAt}`)
  return lines.filter((l) => l !== '' || true).join('\n')
}
