"use client"

import { useEffect, useRef, useState } from "react"

// ---------------------------------------------------------------------------
// SUPPORT-2 — Clients page tab bar: Canonical Clients | Imported Contacts.
//
// A client-only tab control. It flips the `data-active-tab` attribute on the
// nearest `[data-tab-root]` ancestor; the page's server-rendered panes
// (canonical + imported) are shown/hidden by CSS keyed on that attribute. This
// keeps the canonical Clients (ClientManager + ClientAdmin) server-rendered and
// unchanged while adding the imported-contacts mode.
// ---------------------------------------------------------------------------

export type ClientsTab = "canonical" | "imported"

export function ClientsTabBar({
  importedTotal,
}: {
  importedTotal: number
}) {
  const rootRef = useRef<HTMLDivElement>(null)
  const [active, setActive] = useState<ClientsTab>("canonical")

  useEffect(() => {
    rootRef.current?.closest("[data-tab-root]")?.setAttribute("data-active-tab", active)
  }, [active])

  const base =
    "inline-flex min-h-9 items-center rounded-[var(--portal-tab-radius)] px-4 text-[10px] font-medium uppercase tracking-[0.12em] transition"
  const inactive = "border border-transparent text-[var(--portal-navy-soft)] hover:text-[var(--portal-navy)]"
  const activeCls = "border border-[var(--portal-panel-border)] bg-white/70 text-[var(--portal-navy)]"

  return (
    <div ref={rootRef} className="mb-4 flex items-center gap-1">
      <button
        type="button"
        onClick={() => setActive("canonical")}
        className={`${base} ${active === "canonical" ? activeCls : inactive}`}
      >
        Canonical Clients
      </button>
      <button
        type="button"
        onClick={() => setActive("imported")}
        className={`${base} ${active === "imported" ? activeCls : inactive}`}
      >
        Imported Contacts ({importedTotal.toLocaleString()})
      </button>
    </div>
  )
}
