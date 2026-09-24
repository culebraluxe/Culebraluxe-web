// ---------------------------------------------------------------------------
// UI-01 — Operating-surface registry: the single source of truth for
// "which operating surface owns this route" and "what navigation belongs
// under this surface."
//
// Only EXISTING routes are listed — destinations that do not yet exist are
// omitted (future work), never invented to fill slots.
// ---------------------------------------------------------------------------

import type {
  OperatingSurface,
  OperatingSurfaceDefinition,
  SurfaceNavItem,
} from './types'

/** Stable Tier-1 ordering. */
export const OPERATING_SURFACE_ORDER: readonly OperatingSurface[] = [
  'NEXUS',
  'ACCOUNTING',
  'MARKETING',
  'OPS',
  'SUPPORT',
  'TECH',
]

export const OPERATING_SURFACES: Record<
  OperatingSurface,
  OperatingSurfaceDefinition
> = {
  NEXUS: {
    surface: 'NEXUS',
    label: 'CORE',
    description:
      'Real-estate operating environment — people, properties, deals, showings, offers, documents and activity.',
    home: '/portal/dashboard',
    items: [
      { label: 'Cockpit', href: '/portal/dashboard', authority: 'portal.read', entitlement: 'cockpit.read' },
      { label: 'Clients', href: '/portal/clients', authority: 'portal.read', entitlement: 'person.read' },
      { label: 'Projects', href: '/portal/projects', authority: 'portal.read', entitlement: 'project.read' },
      { label: 'Contracts', href: '/portal/deals', authority: 'deal.read', entitlement: 'deal.read' },
      { label: 'Cabinet', href: '/portal/documents', authority: 'deal.read', entitlement: 'vault.read' },
      { label: 'Workflows', href: '/portal/workflows', authority: 'portal.read', entitlement: 'portal.read' },
      { label: 'Forms', href: '/portal/forms', authority: 'deal.read', entitlement: 'form.read' },
      { label: 'Seller Strategy', href: '/portal/core/seller-strategy', authority: 'portal.read', entitlement: 'portal.read' },
    ],
  },
  ACCOUNTING: {
    surface: 'ACCOUNTING',
    label: 'ACCOUNTING',
    description:
      'Brokerage accounting — receivables, expenses, P&L and receipt intake. A simple cash-oriented management view (not QuickBooks).',
    home: '/portal/accounting',
    items: [
      { label: 'Dashboard', href: '/portal/accounting', authority: 'portal.read', entitlement: 'accounting.read' },
      {
        label: 'Receivables',
        href: '/portal/accounting/receivables',
        authority: 'portal.read', entitlement: 'accounting.read',
      },
      {
        label: 'Expenses',
        href: '/portal/accounting/expenses',
        authority: 'portal.read', entitlement: 'accounting.read',
      },
      {
        label: 'P&L Statement',
        href: '/portal/accounting/pnl',
        authority: 'portal.read', entitlement: 'accounting.read',
      },
      {
        label: 'Receipt Scanner',
        href: '/portal/accounting/receipt-scanner',
        authority: 'portal.read', entitlement: 'accounting.read',
      },
    ],
  },
  MARKETING: {
    surface: 'MARKETING',
    label: 'MARKETING',
    description:
      'Outbound listing presence — one canonical property, many channels. HubSpot stays a sibling system, not this ledger.',
    home: '/portal/marketing',
    items: [
      { label: 'Dashboard', href: '/portal/marketing', authority: 'portal.read', entitlement: 'property.read' },
      {
        label: 'Syndication',
        href: '/portal/marketing/syndication',
        authority: 'portal.read', entitlement: 'property.read',
      },
    ],
  },
  OPS: {
    surface: 'OPS',
    label: 'OPPS',
    description:
      'Office tools — Records for bounded property edits, Listing Media for listing photos.',
    home: '/portal/property-admin',
    items: [
      {
        label: 'Records',
        href: '/portal/property-admin',
        authority: 'portal.read', entitlement: 'property.read',
      },
      {
        label: 'Listing Media',
        href: '/portal/property-media',
        authority: 'portal.read', entitlement: 'property.read',
      },
    ],
  },
  TECH: {
    surface: 'TECH',
    label: 'TECH',
    description:
      'Engineering and platform capability — Forge, Story Board, workflow engineering, MQ/replay, integration checkpoints and engineering evidence.',
    minSecurityLevel: 'ROOT',
    accessAuthority: 'tech.access',
    home: '/portal/tech',
    ownedRoutes: ['/portal/tech/framer-ui-lab', '/portal/media-test'],
    items: [
      {
        label: 'Cockpit',
        href: '/portal/tech',
        authority: 'tech.access', entitlement: 'tech.access',
      },
      { label: 'Story Board', href: '/portal/storyboard', authority: 'tech.access', entitlement: 'tech.access' },
      { label: 'UI Lab', href: '/portal/design-lab', authority: 'tech.access', entitlement: 'tech.access' },
    ],
  },
  SUPPORT: {
    surface: 'SUPPORT',
    label: 'SUPPORT',
    description:
      'Technology operations — health, diagnostics, incidents, recovery, releases and runtime/environment.',
    home: '/portal/system-health',
    items: [
      {
        label: 'System Health',
        href: '/portal/system-health',
        authority: 'portal.read', entitlement: 'portal.read',
      },
      {
        label: 'DB Test',
        href: '/portal/db-test',
        authority: 'portal.read', entitlement: 'portal.read',
      },
      {
        label: 'WhatsApp Diagnostic',
        href: '/portal/admin/whatsapp-meta',
        authority: 'portal.read', entitlement: 'portal.read',
      },
      {
        label: 'Security',
        href: '/portal/settings',
        authority: 'settings.read', entitlement: 'security.principal.read',
      },
    ],
  },
}

export function surfaceForPathname(pathname: string): OperatingSurface {
  let best: { surface: OperatingSurface; href: string } | null = null
  for (const surface of OPERATING_SURFACE_ORDER) {
    const definition = OPERATING_SURFACES[surface]
    const owned = [
      ...definition.items.map((item) => item.href),
      ...(definition.ownedRoutes ?? []),
    ]
    for (const href of owned) {
      if (pathname === href || pathname.startsWith(`${href}/`)) {
        if (!best || href.length > best.href.length) {
          best = { surface, href }
        }
      }
    }
  }
  return best?.surface ?? 'NEXUS'
}

export function navigationForSurface(
  surface: OperatingSurface,
): SurfaceNavItem[] {
  return OPERATING_SURFACES[surface].items
}

export function surfaceHome(surface: OperatingSurface): string {
  return OPERATING_SURFACES[surface].home
}

export function surfaceDefinition(
  surface: OperatingSurface,
): OperatingSurfaceDefinition {
  return OPERATING_SURFACES[surface]
}

export function isOperatingSurface(
  value: string | null | undefined,
): value is OperatingSurface {
  return (
    value !== null &&
    value !== undefined &&
    (OPERATING_SURFACE_ORDER as readonly string[]).includes(value)
  )
}
