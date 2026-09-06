'use client'

import Image from 'next/image'
import Link from 'next/link'
import { usePathname } from 'next/navigation'

import type { PortalActorSnapshot } from '@/lib/auth/types'
import {
  CommandPalette,
  type PaletteClient,
  type PaletteDeal,
} from '@/components/portal/command-palette'
import {
  OPERATING_SURFACE_ORDER,
  OPERATING_SURFACES,
  navigationForSurface,
  surfaceForPathname,
  surfaceHome,
} from '@/lib/navigation'
import { hasSecurityLevel } from '@/services/security/level'

// ---------------------------------------------------------------------------
// UI-01 — Operating shell: one application, five operating worlds, ONE
// navigation model on every screen size.
//
//   Top nav (navy header, WITH the logo):
//     CORE | ACCOUNTING | OPPS | SUPPORT | TECH
//     The logo returns to the public CulebraLuxe site. These items are never
//     moved to a second row, never hamburger-only.
//   Submenu  contextual glass rail for the active surface (horizontally
//            scrollable on narrow screens)
//
// The active surface is DERIVED from the current route (longest-prefix match),
// so route changes never reset or corrupt navigation state. SecurityService
// broad levels and legacy authority references only control UI visibility here;
// the Portal layout guard remains the server-side boundary.
// ---------------------------------------------------------------------------

export function OperatingShell({
  actor,
  clients = [],
  deals = [],
  children,
}: {
  actor: PortalActorSnapshot
  clients?: PaletteClient[]
  deals?: PaletteDeal[]
  children: React.ReactNode
}) {
  const pathname = usePathname()
  const activeSurface = surfaceForPathname(pathname)

  const visibleItems = navigationForSurface(activeSurface).filter((item) => {
    const levelVisible =
      !item.minSecurityLevel ||
      hasSecurityLevel(actor.securityLevel, item.minSecurityLevel)
    const authorityVisible =
      !item.authority || actor.authorityCodes.includes(item.authority)
    return levelVisible && authorityVisible
  })

  // Broad SecurityService levels now have a first-class UI hook. Existing
  // authority references remain as entitlement-compatible metadata until the
  // dedicated entitlement story replaces their policy.
  const tier1Items: Array<{
    key: string
    label: string
    href: string
    external: boolean
  }> = [
    {
      key: 'NEXUS',
      label: OPERATING_SURFACES.NEXUS.label,
      href: surfaceHome('NEXUS'),
      external: false,
    },
    ...OPERATING_SURFACE_ORDER.filter((s) => {
      if (s === 'NEXUS') return false
      const def = OPERATING_SURFACES[s]
      const levelVisible =
        !def.minSecurityLevel ||
        hasSecurityLevel(actor.securityLevel, def.minSecurityLevel)
      const authorityVisible =
        !def.accessAuthority ||
        actor.authorityCodes.includes(def.accessAuthority)
      return levelVisible && authorityVisible
    }).map((s) => ({
      key: s,
      label: OPERATING_SURFACES[s].label,
      href: surfaceHome(s),
      external: false,
    })),
  ]

  return (
    <div className="portal-page">
      <header className="sticky top-0 z-20">
        {/* ONE top nav: logo + the five worlds (CORE, ACCOUNTING, OPPS, SUPPORT, TECH) + account. */}
        <div className="border-b border-[var(--portal-gold)]/25 bg-brand-navy text-white">
          <div className="flex min-h-12 items-stretch gap-3 px-3 sm:px-6 lg:px-10">
            <Link
              href="/"
              aria-label="CulebraLuxe home — public site"
              className="me-4 flex flex-none items-center py-2 lg:me-20"
            >
              <Image
                src="/images/culebraluxe-header-logo-test.png"
                alt="CulebraLuxe"
                width={2050}
                height={300}
                priority
                className="h-7 w-auto max-w-[160px] flex-none object-contain sm:h-8 sm:max-w-[200px]"
              />
            </Link>

            <nav aria-label="Operating surface" className="portal-top-nav">
              {tier1Items.map((item) => {
                const active = !item.external && item.key === activeSurface
                return (
                  <Link
                    key={item.key}
                    href={item.href}
                    aria-current={active ? 'page' : undefined}
                    className="top-nav-capsule"
                  >
                    {item.label}
                  </Link>
                )
              })}
            </nav>

            <div className="flex flex-none items-center gap-3 lg:ps-6">
              <Link
                href="/api/auth/signout"
                className="text-[10px] font-light uppercase tracking-[0.2em] text-[var(--portal-ivory)]/60 transition hover:text-[var(--portal-ivory)]"
              >
                Sign out
              </Link>
            </div>
          </div>
        </div>

        {/* Submenu only — the five worlds are in the navy bar above, not here. */}
        <div className="portal-glass-nav">
          <div className="overflow-x-auto px-3 py-2 sm:px-6 lg:px-10 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
            <nav
              aria-label={`${activeSurface} navigation`}
              className="portal-glass-rail"
            >
              {visibleItems.map((item) => {
                const activeItem =
                  pathname === item.href || pathname.startsWith(`${item.href}/`)
                return (
                  <Link
                    key={item.href}
                    href={item.href}
                    aria-current={activeItem ? 'page' : undefined}
                    className="portal-glass-tab"
                  >
                    {item.label}
                  </Link>
                )
              })}
            </nav>
          </div>
        </div>
      </header>

      <main className="px-3 py-4 sm:px-6 lg:px-10 lg:py-5">{children}</main>
      <CommandPalette clients={clients} deals={deals} />
    </div>
  )
}
