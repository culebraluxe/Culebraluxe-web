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
      const response = transport.response as {
        status?: string
        pageFeed?: { body?: { id?: string; permalink_url?: string } }
      } | undefined
      const fbStatus = response?.status ?? 'dry_run'
      const noPhotos = source.photos.length === 0
      // Persist the Graph post id when the Page /feed write succeeded.
      const graphPostId = response?.pageFeed?.body?.id ?? null
      let status: AdapterResult['status'] = 'pending_manual'
      let message: string
      if (fbStatus === 'live') {
        status = 'live'
        message =
          'Page /feed (and catalog when configured) returned 2xx. The Graph post id was saved as the external id.'
      } else if (fbStatus === 'partial') {
        message =
          'Catalog home_listing returned 2xx but the Page feed did not. Confirm the catalog item.'
      } else if (fbStatus === 'failed') {
        status = 'failed'
        message = 'Graph POST attempted and did not return 2xx. Inspect pack.transport.response.'
      } else {
        message = noPhotos
          ? 'No real photos on this listing — Meta cannot ingest it. Add media in Property Admin, then prepare again.'
          : transport.missingEnv.length
            ? 'Dry-run Graph payloads stored. Set META_ACCESS_TOKEN and META_PAGE_ID (catalog id optional) and SYNDICATION_LIVE=true to POST. Marketplace consumer create stays a paste pack.'
            : 'Dry-run stored (SYNDICATION_LIVE not true). Set SYNDICATION_LIVE=true to POST the Page feed.'
      }
      const result = packResult(
        source,
        channel,
        message,
        status,
        [
          'Primary write: POST /{page-id}/feed (Bearer token). Page /feed alone is enough to go live.',
          'Catalog POST /{catalog-id}/home_listings is optional until META_PRODUCT_CATALOG_ID is set.',
          'Marketplace consumer card: still paste from the Page, then confirm the live URL.',
          'Do not post from a personal profile.',
        ].join(' '),
        transport,
      )
      return { ...result, externalId: graphPostId, ok: status !== 'failed' }
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
