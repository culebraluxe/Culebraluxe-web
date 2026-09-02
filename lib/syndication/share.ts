import type { ListingSource } from './types'

// ---------------------------------------------------------------------------
// Slice D — Share blurbs + QR. WhatsApp/SMS one-tap text in ES/EN built from
// the same stored listing facts. Public URL is always culebraluxe.com/listings.
// ---------------------------------------------------------------------------

export type ShareLang = 'es' | 'en'

function money(value: number | null): string {
  if (value == null || !Number.isFinite(value)) return 'Precio a consultar'
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: 0,
  }).format(value)
}

function facts(source: ListingSource, lang: ShareLang): string {
  const parts: string[] = []
  if (source.bedrooms != null) parts.push(lang === 'es' ? `${source.bedrooms} hab` : `${source.bedrooms} bed`)
  if (source.bathrooms != null) parts.push(lang === 'es' ? `${source.bathrooms} baños` : `${source.bathrooms} bath`)
  if (source.squareFeet != null) {
    parts.push(lang === 'es' ? `${source.squareFeet.toLocaleString('en-US')} pie²` : `${source.squareFeet.toLocaleString('en-US')} sqft`)
  }
  return parts.join(' · ')
}

function locationLine(source: ListingSource): string {
  return source.city ? `${source.city}, Puerto Rico` : 'Culebra, Puerto Rico'
}

function publicUrl(source: ListingSource): string {
  return source.publicUrl ?? 'https://culebraluxe.com'
}

export function whatsappBlurb(source: ListingSource, lang: ShareLang): string {
  const price = money(source.listPrice)
  const factsText = facts(source, lang)
  const where = locationLine(source)
  const url = publicUrl(source)
  if (lang === 'es') {
    return [
      `🏠 ${source.name} — ${price}`,
      where,
      factsText,
      '',
      `Detalle y fotos: ${url}`,
      'CulebraLuxe · Bienes Raíces en Culebra',
    ].join('\n')
  }
  return [
    `🏠 ${source.name} — ${price}`,
    where,
    factsText,
    '',
    `Details and photos: ${url}`,
    'CulebraLuxe Real Estate · Culebra, PR',
  ].join('\n')
}

export function smsBlurb(source: ListingSource, lang: ShareLang): string {
  const price = money(source.listPrice)
  const factsText = facts(source, lang)
  const url = publicUrl(source)
  const marker = lang === 'es' ? 'CulebraLuxe' : 'CulebraLuxe'
  return `${source.name} ${price} — ${factsText}. ${url} (${marker})`
}

export function waMeUrl(source: ListingSource, lang: ShareLang): string {
  return `https://wa.me/?text=${encodeURIComponent(whatsappBlurb(source, lang))}`
}

/** Blurb length is fine to fit a WhatsApp/SMS share without truncation. */
export function shareText(source: ListingSource, lang: ShareLang): string {
  return whatsappBlurb(source, lang)
}
