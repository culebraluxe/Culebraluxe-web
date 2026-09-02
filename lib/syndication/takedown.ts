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
