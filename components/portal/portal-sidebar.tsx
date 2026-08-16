"use client"

import Link from "next/link"
import { usePathname } from "next/navigation"

const navigation = [
  {
    label: "Dashboard",
    href: "/portal/dashboard",
  },
  {
    label: "Deals Portfolio",
    href: "/portal/deals",
  },
  {
    label: "Client Manager",
    href: "/portal/clients",
  },
]

export function PortalSidebar() {
  const pathname = usePathname()

  return (
    <aside className="hidden w-64 shrink-0 border-r border-[var(--portal-border)] bg-[var(--portal-navy)] text-white lg:flex lg:flex-col">
      <div className="border-b border-white/10 px-8 py-8">
        <Link href="/" className="block">
          <div className="font-serif text-2xl font-light uppercase tracking-[0.08em]">
            CULEBRALUXE
          </div>

          <div className="mt-1 text-[10px] font-light uppercase tracking-[0.32em] text-white/45">
            Private Portal
          </div>
        </Link>
      </div>

      <nav className="flex-1 px-4 py-8">
        <div className="space-y-1">
          {navigation.map((item) => {
            const active = pathname.startsWith(item.href)

            return (
              <Link
                key={item.href}
                href={item.href}
                className={[
                  "block rounded-sm px-4 py-3 text-sm font-light transition-colors",
                  active
                    ? "bg-white/12 text-white"
                    : "text-white/55 hover:bg-white/7 hover:text-white",
                ].join(" ")}
              >
                {item.label}
              </Link>
            )
          })}
        </div>
      </nav>

      <div className="border-t border-white/10 px-8 py-7">
        <div className="text-[10px] font-light uppercase tracking-[0.28em] text-white/40">
          Culebra · Puerto Rico
        </div>

        <div className="mt-3 text-xs font-light leading-5 text-white/45">
          Clients, properties and opportunities.
        </div>
      </div>
    </aside>
  )
}