import Link from "next/link"

import type { PortalActorSnapshot } from "@/lib/auth/types"

// AUTH-02: receives the server-resolved actor snapshot (cosmetic UI projection
// only — the security boundary is the server-side layout guard).
export function PortalHeader({ actor }: { actor: PortalActorSnapshot }) {
  return (
    <header className="border-b border-white/10 bg-[var(--portal-navy)] text-white">
      <div className="flex min-h-20 items-center justify-between gap-6 px-6 lg:px-10 xl:px-14">
        <div className="min-w-0 flex-1">
          <div className="max-w-xl">
            <input
              type="search"
              placeholder="Search clients, properties, deals..."
              className="w-full border-b border-white/20 bg-transparent py-2 text-sm font-light text-white outline-none placeholder:text-white/40 focus:border-white/60"
            />
          </div>
        </div>

        <div className="flex shrink-0 items-center gap-6">
          <button
            type="button"
            className="text-xs font-light uppercase tracking-[0.16em] text-white/55 transition hover:text-white"
          >
            Notifications
          </button>

          <Link
            href="/api/auth/signout"
            className="text-xs font-light uppercase tracking-[0.16em] text-white/55 transition hover:text-white"
          >
            Sign out
          </Link>

          <div className="hidden border-l border-white/15 pl-6 text-right sm:block">
            <div className="font-serif text-base font-light uppercase tracking-[0.08em] text-white">
              CULEBRALUXE
            </div>

            <div className="mt-1 text-[9px] font-light uppercase tracking-[0.25em] text-white/45">
              {actor.displayName}
            </div>
          </div>
        </div>
      </div>
    </header>
  )
}
