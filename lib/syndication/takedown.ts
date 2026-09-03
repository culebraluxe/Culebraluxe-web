import type { ListingSource } from './types'

// ---------------------------------------------------------------------------
// Task 3 — Off-market takedown copy kit (ES/EN). Copy for Lisa to send/use when a
// listing leaves the market. Advises removing/stopping off-site paths and
// updating Matrix. This app never auto-deletes a third-party ad.
// ---------------------------------------------------------------------------

export function takedownTextEn(source: ListingSource): string {
  return [
    `${source.name} is now off the market.`,
    '',
    'Please stop/remove the Facebook post and the Clasificados ad, and mark the listing withdrawn in Stellar Matrix so the feed portals update.',
    'Keep your CulebraLuxe agent records as-is — do not delete listing history.',
  ].join('\n')
}

export function takedownTextEs(source: ListingSource): string {
  return [
    `${source.name} ya no está en el mercado.`,
    '',
    'Favor detener/retirar la publicación de Facebook y el anuncio de Clasificados, y marcar la propiedad como retirada en Stellar Matrix para que los portales se actualicen.',
    'Mantén los registros de CulebraLuxe tal cual — no borres el historial de la propiedad.',
  ].join('\n')
}

/** Shareable off-market / just-sold blurb. Unpublished listings point at inventory, not a dead page. */
function browseUrl(source: ListingSource): string {
  return source.isPublished && source.publicUrl ? source.publicUrl : 'https://culebraluxe.com'
}

export function offMarketShareEn(source: ListingSource): string {
  return `${source.name} is now off the market. Thank you for your interest — explore current CulebraLuxe listings: ${browseUrl(source)}`
}

export function offMarketShareEs(source: ListingSource): string {
  return `${source.name} ya no está en el mercado. Gracias por tu interés — mira las propiedades actuales de CulebraLuxe: ${browseUrl(source)}`
}
