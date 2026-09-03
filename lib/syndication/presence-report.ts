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
  stellarPrepared: boolean
  stellarConfirmed: boolean
  facebookStatus: string
  facebookUrl: string | null
  facebookPrepared: boolean
  facebookConfirmed: boolean
  clasificadosStatus: string
  clasificadosUrl: string | null
  clasificadosPrepared: boolean
  clasificadosConfirmed: boolean
  sightings: PresenceNetworkLine[]
  daysOnMarket: number | null
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

  // Days on market from the earliest placement that ever went live (ISO).
  let daysOnMarket: number | null = null
  const isoDates = rows
    .map((r) => r.publishedAtIso)
    .filter((d): d is string => Boolean(d))
  if (isoDates.length > 0) {
    const earliest = Math.min(...isoDates.map((d) => new Date(d).getTime()))
    if (Number.isFinite(earliest)) {
      daysOnMarket = Math.max(0, Math.floor((Date.now() - earliest) / 86_400_000))
    }
  }

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
    stellarPrepared: !!stellar,
    stellarConfirmed,
    facebookStatus: facebookConfirmed
      ? facebook?.status === 'live' && facebook?.externalUrl
        ? 'Page / Marketplace URL confirmed'
        : 'Pack ready — paste the live post URL to confirm'
      : facebook
        ? 'Pack ready (not yet confirmed)'
        : 'Not prepared yet',
    facebookUrl: facebook?.externalUrl ?? null,
    facebookPrepared: !!facebook,
    facebookConfirmed,
    clasificadosStatus: clasificadosConfirmed
      ? 'Clasificados live URL confirmed'
      : clasificados
        ? 'Clasificados pack ready — paste the live ad URL'
        : 'Not prepared (optional)',
    clasificadosUrl: clasificados?.externalUrl ?? null,
    clasificadosPrepared: !!clasificados,
    clasificadosConfirmed,
    sightings: sightings
      .filter((s) => s.propertyId === source.id)
      .map((s) => ({ network: s.network, url: s.url, notes: s.notes })),
    daysOnMarket,
    disclaimer: DISCLAIMER,
    generatedAt: new Intl.DateTimeFormat('en-US', {
      timeZone: 'America/Puerto_Rico',
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(new Date()),
  }
}

const DISCLAIMER_ES =
  'Los portales (Zillow, Realtor.com, Homes.com) se actualizan desde el feed de Stellar MLS. ' +
  'Esto lista URLs confirmadas u observadas — no es un registro de publicación. Nada de esto fue "publicado en Zillow" por esta app.'

export type ReportLang = 'en' | 'es'

function stellarLine(report: PresenceReport, es: boolean): string {
  if (report.stellarConfirmed) {
    return es ? 'En Stellar — los portales siguen el feed' : 'In Stellar — portals follow the feed'
  }
  if (report.stellarPrepared) {
    return es
      ? 'Pack preparado — ingrésalo en Matrix y confirma el # MLS'
      : 'Pack prepared — enter in Matrix, then paste the MLS#'
  }
  return es ? 'Aún no en Matrix' : 'Not in Matrix yet'
}

function facebookLine(report: PresenceReport, es: boolean): string {
  if (report.facebookUrl) return report.facebookUrl
  if (report.facebookConfirmed) {
    return es ? 'Página / Marketplace URL confirmada' : 'Page / Marketplace URL confirmed'
  }
  if (report.facebookPrepared) {
    return es
      ? 'Pack listo — pega la URL de la publicación para confirmar'
      : 'Pack ready — paste the live post URL to confirm'
  }
  return es ? 'No preparado aún' : 'Not prepared yet'
}

function clasificadosLine(report: PresenceReport, es: boolean): string {
  if (report.clasificadosUrl) return report.clasificadosUrl
  if (report.clasificadosConfirmed) {
    return es ? 'Clasificados URL activa confirmada' : 'Clasificados live URL confirmed'
  }
  if (report.clasificadosPrepared) {
    return es
      ? 'Pack de Clasificados listo — pega la URL del anuncio'
      : 'Clasificados pack ready — paste the live ad URL'
  }
  return es ? 'No preparado (opcional)' : 'Not prepared (optional)'
}

/** Plain-text rendering for Copy and print, in English or Spanish. */
export function presenceReportText(report: PresenceReport, lang: ReportLang = 'en'): string {
  const es = lang === 'es'
  const lines: string[] = []
  lines.push(`${report.propertyName} — ${report.priceLabel}`)
  lines.push(report.city)
  lines.push(report.factsLine)
  if (report.daysOnMarket != null) lines.push(`${es ? 'Días en el mercado' : 'Days on market'}: ${report.daysOnMarket}`)
  lines.push('')
  lines.push(report.sitePublished && report.siteUrl
    ? `${es ? 'En culebraluxe.com' : 'On culebraluxe.com'}: ${es ? 'Publicada' : 'Live'}`
    : `${es ? 'En culebraluxe.com' : 'On culebraluxe.com'}: ${es ? 'No publicada' : 'Not published'}`)
  if (report.siteUrl) lines.push(`  ${report.siteUrl}`)
  lines.push(stellarLine(report, es))
  if (report.stellarMls) lines.push(`  MLS# ${report.stellarMls}`)
  lines.push(`${es ? 'Facebook' : 'Facebook'}: ${facebookLine(report, es)}`)
  lines.push(`${es ? 'Clasificados' : 'Clasificados'}: ${clasificadosLine(report, es)}`)
  if (report.sightings.length === 0) {
    lines.push(es ? 'Zillow / Realtor.com: No observada aún.' : 'Zillow / Realtor.com: Not observed yet.')
  } else {
    lines.push(es ? 'Vistas en portales:' : 'Observed on portals:')
    for (const s of report.sightings) {
      lines.push(`  ${es ? 'En' : ''} ${s.network}${s.url ? ` · ${s.url}` : ''}${s.notes ? ` · ${s.notes}` : ''}`)
    }
  }
  lines.push('', es ? DISCLAIMER_ES : report.disclaimer, `${es ? 'Generado' : 'Generated'} ${report.generatedAt}`)
  return lines.join('\n')
}
