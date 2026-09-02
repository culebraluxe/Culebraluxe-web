import { CHANNEL_CATALOG, type SyndicationChannel } from './channels'
import { buildListingPack } from './pack'
import type { AdapterResult, ListingSource } from './types'

const PASTE_TARGETS: Partial<Record<SyndicationChannel, string>> = {
  clasificados: 'https://www.clasificadosonline.com/Usuarios.asp',
  facebook_marketplace: 'https://www.facebook.com/marketplace/create/item',
  zillow_fsbo: 'https://www.zillow.com/sell/for-sale-by-owner/',
}

function packResult(
  source: ListingSource,
  channel: SyndicationChannel,
  message: string,
  status: AdapterResult['status'] = 'ready',
  instructions?: string,
): AdapterResult {
  const def = CHANNEL_CATALOG[channel]
  const pack = buildListingPack(source, def)
  pack.pasteTargetUrl = PASTE_TARGETS[channel] ?? null
  if (instructions) pack.instructions = instructions
  return {
    ok: true,
    mode: def.mode,
    status,
    pack,
    message,
    ttlDays: def.defaultTtlDays,
  }
}

export function runAdapter(
  source: ListingSource,
  channel: SyndicationChannel,
): AdapterResult {
  const def = CHANNEL_CATALOG[channel]

  switch (channel) {
    case 'culebraluxe':
      return {
        ok: source.isPublished,
        mode: 'api',
        status: source.isPublished ? 'live' : 'draft',
        pack: buildListingPack(source, def),
        message: source.isPublished
          ? 'Live on culebraluxe.com. This flag is property.is_published — change it in Property Admin.'
          : 'Not public on the CulebraLuxe site yet. Publish the property first.',
        ttlDays: null,
      }
    case 'clasificados':
      return packResult(
        source,
        channel,
        'Pack ready. Clasificados has no publisher API — paste once, then confirm the live URL so the ledger can close the round trip.',
        'pending_manual',
        [
          'Log in to ClasificadosOnline with the brokerage account.',
          'Nueva clasificación → Bienes Raíces → Venta.',
          'Paste title (ES) and body (ES). Attach downloaded photos — do not hotlink culebraluxe.com.',
          'One new sale ad every few days. Sale ads expire ~40 days.',
          'When the ad is live, paste the public URL back here and mark confirmed.',
        ].join(' '),
      )
    case 'facebook_marketplace':
      return packResult(
        source,
        channel,
        'Pack ready for the CulebraLuxe Page. Graph publish stays stubbed until the Page token is wired — Business + Tech Partner is already in place.',
        'pending_manual',
        [
          'Create the listing from the CulebraLuxe Facebook Page, not a personal profile.',
          'Paste title (EN) and body (EN). Price and location from the pack.',
          'Upload the same photo set used on the site.',
          'After it is live, paste the Marketplace URL and confirm.',
          'Later adapter: Marketing API / catalog item using the Tech Partner app.',
        ].join(' '),
      )
    case 'zillow_fsbo':
      return packResult(
        source,
        channel,
        'Pack ready for a Zillow By-Owner card. This will not appear as an MLS listing on Realtor.com.',
        'pending_manual',
        [
          'Use Zillow For Sale By Owner only as a fallback card.',
          'Paste title, price, and the public CulebraLuxe URL.',
          'Full Zillow / Realtor.com search inventory requires an MLS feed (Amplia or PRAR).',
          'Confirm the Zillow listing URL here when it is live.',
        ].join(' '),
      )
    case 'pr_mls':
      return {
        ok: false,
        mode: 'blocked',
        status: 'draft',
        pack: buildListingPack(source, def),
        message: def.notes,
        ttlDays: null,
      }
    case 'realtor_com':
      return {
        ok: false,
        mode: 'blocked',
        status: 'draft',
        pack: buildListingPack(source, def),
        message: def.notes,
        ttlDays: null,
      }
    case 'amplia_mls':
      return {
        ok: true,
        mode: 'mls',
        status: 'draft',
        pack: buildListingPack(source, def),
        message:
          'Queued as a stub. Confirm Amplia membership and their portal feed in writing before marking live.',
        ttlDays: null,
      }
    case 'hubspot':
      return {
        ok: true,
        mode: 'api',
        status: 'draft',
        pack: buildListingPack(source, def),
        message:
          'Queued as a stub. HubSpot stays side-by-side — listing object / campaign sync is not wired.',
        ttlDays: null,
      }
  }
}
