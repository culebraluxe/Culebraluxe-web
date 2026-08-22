// AUTH-02 Portal navigation projection, filtered by authority for cosmetic UI
// gating. Framework-free on purpose so the filter is unit-testable.
//
// UI hiding only — never the security boundary. Every /portal route is
// enforced server-side by the Portal layout guard (portal.read) and the
// settings guard (settings.read); nav items here simply stop being rendered
// when the actor lacks the mapped authority.

import type { AuthorityCode } from './types'

export type PortalNavItem = {
  label: string
  href: string
  authority: AuthorityCode
}

export type PortalNavGroup = {
  label: string
  items: PortalNavItem[]
}

// Matches the coarse authority map (docs/auth-command-map.md): operational
// views → portal.read, deal listing/workspaces → deal.read, settings → settings.read.
export const PORTAL_NAVIGATION: PortalNavGroup[] = [
  {
    label: 'Work',
    items: [
      { label: 'Dashboard', href: '/portal/dashboard', authority: 'portal.read' },
      { label: 'Attention', href: '/portal/attention', authority: 'portal.read' },
      { label: 'Needs Review', href: '/portal/needs-review', authority: 'portal.read' },
      { label: 'Activity', href: '/portal/activity', authority: 'portal.read' },
    ],
  },
  {
    label: 'Operations & Reporting',
    items: [
      { label: 'Deals', href: '/portal/deals', authority: 'deal.read' },
      { label: 'Workflows', href: '/portal/workflows', authority: 'portal.read' },
      { label: 'Showings', href: '/portal/showings', authority: 'portal.read' },
      { label: 'Clients', href: '/portal/clients', authority: 'portal.read' },
      { label: 'Property Admin', href: '/portal/property-admin', authority: 'portal.read' },
      { label: 'Media Audit', href: '/portal/media-admin', authority: 'portal.read' },
      { label: 'Reporting', href: '/portal/reporting', authority: 'portal.read' },
      { label: 'Story Board', href: '/portal/storyboard', authority: 'portal.read' },
      { label: 'Command Center', href: '/portal/command-center', authority: 'portal.read' },
      { label: 'Command Console', href: '/portal/command-console', authority: 'portal.read' },
      { label: 'Identity Quality', href: '/portal/identity-quality', authority: 'portal.read' },
      { label: 'System Health', href: '/portal/system-health', authority: 'portal.read' },
      { label: 'Settings', href: '/portal/settings', authority: 'settings.read' },
    ],
  },
]

export function filterPortalNavigation(
  authorityCodes: string[],
): PortalNavGroup[] {
  return PORTAL_NAVIGATION.map((group) => ({
    ...group,
    items: group.items.filter((item) =>
      authorityCodes.includes(item.authority),
    ),
  })).filter((group) => group.items.length > 0)
}
