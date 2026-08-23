'use client'

import Image from 'next/image'
import Link from 'next/link'
import { usePathname } from 'next/navigation'

import type { PortalActorSnapshot } from '@/lib/auth/types'
import {
  OPERATING_SURFACE_ORDER,
  OPERATING_SURFACES,
  navigationForSurface,
  surfaceForPathname,
  surfaceHome,
} from '@/lib/navigation'

// ---------------------------------------------------------------------------
// UI-01 — Operating shell: one application, four operating worlds, ONE
// navigation model on every screen size.
//
//   Tier 1  NEXUS | MAIN | OPPS | SUPPORT | TECH   (sticky, always visible —
//                                          never a hamburger-only model; MAIN
//                                          returns to the public CulebraLuxe site)
//   Tier 2  contextual tab rail for the active surface (horizontally scrollable
//          on narrow screens)
//
// The active surface is DERIVED from the current route (longest-prefix match),
// so route changes never reset or corrupt navigation state. Cosmetic UI gating
// only — the security boundary stays server-side (the portal layout guard).
// ---------------------------------------------------------------------------

export function OperatingShell({
  actor,
  children,
}: {
  actor: PortalActorSnapshot
  children: React.ReactNode
}) {
  const pathname = usePathname()
  const activeSurface = surfaceForPathname(pathname)

  // Cosmetic filtering by the actor's existing authority codes (UI hiding is
  // never the security boundary — the server-side guard enforces portal.read).
  const visibleItems = navigationForSurface(activeSurface).filter(
    (item) => !item.authority || actor.authorityCodes.includes(item.authority),
  )

  // Tier-1 chrome: the four operating surfaces (NEXUS | OPPS | SUPPORT | TECH)
  // plus MAIN, which returns to the public CulebraLuxe site. Exact display
  // order: NEXUS | MAIN | OPPS | SUPPORT | TECH.
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
    { key: 'MAIN', label: 'MAIN', href: '/', external: true },
    ...OPERATING_SURFACE_ORDER.filter((s) => s !== 'NEXUS').map((s) => ({
      key: s,
      label: OPERATING_SURFACES[s].label,
      href: surfaceHome(s),
      external: false,
    })),
  ]

  return (
    <div className="portal-page">
      {/* Brand / account row — logo stays; navy bar is the brand lockup. */}
      <header className="border-b border-[var(--portal-gold)]/25 bg-brand-navy text-white">
        <div className="flex min-h-12 items-center justify-between gap-4 px-4 sm:px-6 lg:px-10">
          <Link
            href="/portal/dashboard"
            aria-label="CulebraLuxe home"
            className="flex h-7 w-[250px] flex-none items-center"
          >
            {/* Exact main-site logo asset — brand must match the public site. */}
            <Image
              src="/images/culebraluxe-header-logo-test.png"
              alt="CulebraLuxe"
              width={2050}
              height={300}
              priority
              className="h-9 max-h-9 w-auto max-w-full flex-none object-contain"
            />
          </Link>

          <div className="flex shrink-0 items-center gap-4">
            <span className="hidden text-[10px] font-light uppercase tracking-[0.22em] text-[var(--portal-ivory)]/70 sm:inline">
              {actor.displayName}
            </span>
            <Link
              href="/api/auth/signout"
              className="text-[10px] font-light uppercase tracking-[0.2em] text-[var(--portal-ivory)]/60 transition hover:text-[var(--portal-ivory)]"
            >
              Sign out
            </Link>
          </div>
        </div>
      </header>

      {/* Tier 1 stays fully visible: NEXUS | MAIN | OPPS | SUPPORT | TECH.
          Tier 2 is the glass submenu capsule. */}
      <div className="portal-glass-nav sticky top-0 z-20">
        <nav
          aria-label="Operating surface"
          className="grid grid-cols-5"
        >
          {tier1Items.map((item) => {
            const active = !item.external && item.key === activeSurface
            return (
              <Link
                key={item.key}
                href={item.href}
                aria-current={active ? 'page' : undefined}
                className={[
                  'flex min-h-12 items-center justify-center border-b-2 px-0.5 text-[10px] font-light uppercase tracking-[0.12em] transition-colors sm:text-xs sm:tracking-[0.2em]',
                  active
                    ? 'border-[var(--portal-gold)] text-[var(--portal-navy)]'
                    : 'border-transparent text-black/45 hover:text-[var(--portal-navy)]',
                ].join(' ')}
              >
                {item.label}
              </Link>
            )
          })}
        </nav>

        <div className="overflow-x-auto px-4 pb-3 pt-2 sm:px-6 lg:px-10 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
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

      <main className="px-4 py-6 sm:px-6 lg:px-10 lg:py-10">{children}</main>
    </div>
  )
}
