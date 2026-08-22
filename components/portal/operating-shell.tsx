'use client'

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
//   Tier 1  NEXUS | OPS | TECH | SUPPORT   (sticky, always visible — never a
//                                          hamburger-only model)
//   Tier 2  contextual nav for the active surface (horizontally scrollable on
//          phone; wraps on desktop)
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

  return (
    <div className="min-h-screen bg-[var(--portal-bg)] text-[var(--portal-text)]">
      {/* Brand / account row (shallow chrome) */}
      <header className="border-b border-white/10 bg-[var(--portal-navy)] text-white">
        <div className="flex min-h-12 items-center justify-between gap-4 px-4 sm:px-6 lg:px-10">
          <Link
            href="/portal/dashboard"
            className="font-serif text-base font-light uppercase tracking-[0.08em] text-white transition hover:text-white/80"
          >
            CulebraLuxe
          </Link>

          <div className="flex shrink-0 items-center gap-4">
            <span className="hidden text-[10px] font-light uppercase tracking-[0.22em] text-white/50 sm:inline">
              {actor.displayName}
            </span>
            <Link
              href="/api/auth/signout"
              className="text-[10px] font-light uppercase tracking-[0.2em] text-white/60 transition hover:text-white"
            >
              Sign out
            </Link>
          </div>
        </div>
      </header>

      {/* Tier 1 + Tier 2 — sticky operating navigation */}
      <div className="sticky top-0 z-20 border-b border-[var(--portal-border)] bg-[var(--portal-bg)]">
        <nav
          aria-label="Operating surface"
          className="grid grid-cols-4"
        >
          {OPERATING_SURFACE_ORDER.map((surface) => {
            const active = surface === activeSurface
            const definition = OPERATING_SURFACES[surface]
            return (
              <Link
                key={surface}
                href={surfaceHome(surface)}
                aria-current={active ? 'page' : undefined}
                className={[
                  'flex min-h-12 items-center justify-center border-b-2 text-xs font-light uppercase tracking-[0.2em] transition-colors',
                  active
                    ? 'border-[#c6a15b] text-[var(--portal-navy)]'
                    : 'border-transparent text-black/45 hover:text-[var(--portal-navy)]',
                ].join(' ')}
              >
                {definition.label}
              </Link>
            )
          })}
        </nav>

        <div className="overflow-x-auto">
          <nav
            aria-label={`${activeSurface} navigation`}
            className="flex items-center gap-1 px-3 py-2 lg:flex-wrap"
          >
            {visibleItems.map((item) => {
              const activeItem =
                pathname === item.href || pathname.startsWith(`${item.href}/`)
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  aria-current={activeItem ? 'page' : undefined}
                  className={[
                    'whitespace-nowrap rounded-sm px-3 py-2 text-[11px] font-light uppercase tracking-[0.14em] transition-colors',
                    activeItem
                      ? 'bg-[var(--portal-navy)] text-white'
                      : 'text-black/50 hover:bg-black/5 hover:text-[var(--portal-navy)]',
                  ].join(' ')}
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
