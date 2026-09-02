import type { ChannelDefinition } from './channels'
import type { ListingPack, ListingSource } from './types'

function money(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return 'Precio a consultar'
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: 0,
  }).format(value)
}

function facts(source: ListingSource): string {
  const parts: string[] = []
  if (source.bedrooms != null) parts.push(`${source.bedrooms} hab`)
  if (source.bathrooms != null) parts.push(`${source.bathrooms} baños`)
  if (source.squareFeet != null) {
    parts.push(`${source.squareFeet.toLocaleString('en-US')} pie²`)
  }
  if (source.propertyType) parts.push(source.propertyType)
  return parts.join(' · ')
}

export function photoHintFor(source: ListingSource): string {
  const photos = source.photos ?? []
  if (photos.length === 0) {
    return source.imageCount > 0
      ? `${source.imageCount} photos on the listing. Download from Property Media, do not hotlink.`
      : 'No photos attached yet — add media before posting.'
  }
  const listed = photos
    .slice(0, 5)
    .map((photo, index) => `${index + 1}. ${photo.url}`)
    .join('\n')
  const more = photos.length > 5 ? `\n+${photos.length - 5} more in Property Media.` : ''
  return `${photos.length} photos. Download these files — do not hotlink.\n${listed}${more}`
}

export function buildListingPack(
  source: ListingSource,
  channel: ChannelDefinition,
): ListingPack {
  const locationLine =
    source.location ??
    [source.neighborhood, source.city].filter(Boolean).join(', ') ??
    'Culebra, Puerto Rico'

  const priceLabel = money(source.listPrice)
  const factsLine = facts(source)
  const contactLine = [
    source.listingAgentName,
    source.listingAgentPhone,
    source.listingAgentEmail,
  ]
    .filter(Boolean)
    .join(' · ')

  const bodyEs = [
    source.shortDescription ?? source.publicRemarks ?? source.name,
    factsLine,
    locationLine,
    source.publicUrl ? `Detalle y fotos: ${source.publicUrl}` : null,
    contactLine ? `Contacto: ${contactLine}` : null,
  ]
    .filter(Boolean)
    .join('\n\n')

  const bodyEn = [
    source.publicRemarks ?? source.shortDescription ?? source.name,
    factsLine,
    locationLine,
    source.publicUrl ? `Details and photos: ${source.publicUrl}` : null,
    contactLine ? `Contact: ${contactLine}` : null,
  ]
    .filter(Boolean)
    .join('\n\n')

  return {
    channel: channel.id,
    titleEs: `${source.name} — ${priceLabel}`,
    titleEn: `${source.name} — ${priceLabel}`,
    bodyEs,
    bodyEn,
    priceLabel,
    locationLine,
    factsLine,
    publicUrl: source.publicUrl,
    contactLine,
    photoHint: photoHintFor(source),
    instructions: channel.notes,
    pasteTargetUrl: null,
    transport: null,
  }
}
