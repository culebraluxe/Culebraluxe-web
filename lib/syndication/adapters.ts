import { CHANNEL_CATALOG, type SyndicationChannel } from './channels'
import { facebookTransportPlan, maybePostFacebook } from './facebook'
import { hubspotTransportPlan, maybePostHubSpot } from './hubspot'
import { buildListingPack } from './pack'
import { stellarTransportPlan } from './stellar'
import type { AdapterResult, ListingPack, ListingSource, TransportAttempt } from './types'

const PASTE_TARGETS: Partial<Record<SyndicationChannel, string>> = {
  clasificados: 'https://www.clasificadosonline.com/Usuarios.asp',
  facebook_marketplace: 'https://www.facebook.com/marketplace/create/item',
  zillow_fsbo: 'https://www.zillow.com/sell/for-sale-by-owner/',
  stellar_mls: 'https://www.stellarmls.com/prar-en',
}

function withTransport(pack: ListingPack, transport: TransportAttempt | null): ListingPack {
  return { ...pack, transport }
}

function packResult(
  source: ListingSource,
  channel: SyndicationChannel,
  message: string,
  status: AdapterResult['status'] = 'ready',
  instructions?: string,
  transport?: TransportAttempt | null,
): AdapterResult {
  const def = CHANNEL_CATALOG[channel]
  const pack = withTransport(buildListingPack(source, def), transport ?? null)
  pack.pasteTargetUrl = PASTE_TARGETS[channel] ?? null
  if (instructions) pack.instructions = instructions
  return {
    ok: true,
    mode: def.mode,
    status,
    pack,
    message,
    ttlDays: def.defaultTtlDays,
    transport: transport ?? null,
  }
}

export async function runAdapter(
  source: ListingSource,
  channel: SyndicationChannel,
): Promise<AdapterResult> {
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
        'Pack ready. Clasificados has no publisher API — paste once, then confirm the live URL.',
        'pending_manual',
        [
          'Log in to ClasificadosOnline with the brokerage account.',
          'Nueva clasificación → Bienes Raíces → Venta.',
          'Paste title (ES) and body (ES). Attach downloaded photos — do not hotlink culebraluxe.com.',
          'One new sale ad every few days. Sale ads expire ~40 days.',
          'When the ad is live, paste the public URL back here and mark confirmed.',
        ].join(' '),
      )
    case 'facebook_marketplace': {
      const planned = facebookTransportPlan(source)
      const transport = await maybePostFacebook(planned)
      const posted =
        transport.response &&
        typeof transport.response.httpStatus === 'number' &&
        transport.response.httpStatus < 300
      return packResult(
        source,
        channel,
        posted
          ? 'Graph home_listing POST returned 2xx. Confirm the catalog item and paste a Marketplace URL if you also posted manually.'
          : transport.dryRun
            ? 'Dry-run Graph payloads stored. Set META_ACCESS_TOKEN, META_PRODUCT_CATALOG_ID, META_PAGE_ID and SYNDICATION_LIVE=true to POST. Marketplace consumer create stays a paste pack.'
            : 'Graph POST attempted. Inspect pack.transport.response.',
        posted ? 'live' : 'pending_manual',
        [
          'Preferred automated path: POST /{catalog-id}/home_listings (catalog ads + Commerce).',
          'Reliable Page path: POST /{page-id}/feed with the listing URL.',
          'Marketplace consumer card: still paste from the Page, then confirm the live URL.',
          'Do not post from a personal profile.',
        ].join(' '),
        transport,
      )
    }
    case 'stellar_mls': {
      const transport = stellarTransportPlan(source)
      return packResult(
        source,
        channel,
        'RESO Property payload + Matrix distribution checklist ready. Stellar has no broker write API — enter once in Matrix, then confirm the MLS number.',
        'pending_manual',
        [
          'Open Stellar Matrix under the CulebraLuxe office.',
          'Enter the RESO fields from the stored payload (price, beds, baths, Culebra, remarks).',
          'Upload photos from Property Media.',
          'On the Realtor tab set Listing Distribution: Realtor.com, Homes.com, Homesnap, ListHub.',
          'Confirm the Matrix listing ID / public portal URL back here.',
        ].join(' '),
        transport,
      )
    }
    case 'zillow_fsbo':
      return packResult(
        source,
        channel,
        'Pack ready for a Zillow By-Owner card. Prefer Stellar syndication instead.',
        'pending_manual',
        [
          'Use Zillow For Sale By Owner only if the Stellar feed has not appeared yet.',
          'Paste title, price, and the public CulebraLuxe URL.',
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
          'Queued as a stub. Flagship path is Stellar. Confirm Amplia portal feed in writing before marking live.',
        ttlDays: null,
      }
    case 'hubspot': {
      const planned = hubspotTransportPlan(source)
      const transport = await maybePostHubSpot(planned)
      const posted =
        transport.response &&
        typeof transport.response.httpStatus === 'number' &&
        transport.response.httpStatus < 300
      return packResult(
        source,
        channel,
        posted
          ? 'HubSpot CRM object created.'
          : transport.dryRun
            ? 'HubSpot listing payload stored (dry-run). Set HUBSPOT_ACCESS_TOKEN and SYNDICATION_LIVE=true to POST.'
            : 'HubSpot POST attempted. Inspect pack.transport.response.',
        posted ? 'live' : 'draft',
        'Custom object type defaults to p_listings. Override with HUBSPOT_LISTING_OBJECT.',
        transport,
      )
    }
  }
}
