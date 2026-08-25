'use client'

import { useEffect, useMemo, useRef, useState } from 'react'
import { useRouter } from 'next/navigation'

import {
  OPERATING_SURFACE_ORDER,
  OPERATING_SURFACES,
} from '@/lib/navigation'

export type PaletteClient = { id: string; name: string }
export type PaletteDeal = { id: string; name: string; client: string }

type PaletteItem = {
  id: string
  label: string
  detail: string
  href: string
  group: 'Go to' | 'People' | 'Deals'
}

function navItems(): PaletteItem[] {
  const items: PaletteItem[] = []
  for (const surface of OPERATING_SURFACE_ORDER) {
    const def = OPERATING_SURFACES[surface]
    items.push({
      id: `surface-${surface}`,
      label: def.label,
      detail: def.description,
      href: def.home,
      group: 'Go to',
    })
    for (const item of def.items) {
      items.push({
        id: item.href,
        label: item.label,
        detail: def.label,
        href: item.href,
        group: 'Go to',
      })
    }
  }
  items.push({
    id: 'main-site',
    label: 'Public site',
    detail: 'CulebraLuxe.com',
    href: '/',
    group: 'Go to',
  })
  return items
}

export function CommandPalette({
  clients,
  deals,
}: {
  clients: PaletteClient[]
  deals: PaletteDeal[]
}) {
  const router = useRouter()
  const inputRef = useRef<HTMLInputElement>(null)
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [active, setActive] = useState(0)

  const catalog = useMemo<PaletteItem[]>(() => {
    const people = clients.map((client) => ({
      id: `person-${client.id}`,
      label: client.name,
      detail: 'Client',
      href: `/portal/clients/${client.id}`,
      group: 'People' as const,
    }))
    const dealItems = deals.map((deal) => ({
      id: `deal-${deal.id}`,
      label: deal.name,
      detail: deal.client,
      href: `/portal/deals/${deal.id}`,
      group: 'Deals' as const,
    }))
    return [...navItems(), ...people, ...dealItems]
  }, [clients, deals])

  const results = useMemo(() => {
    const needle = query.trim().toLowerCase()
    if (!needle) return catalog.slice(0, 12)
    return catalog
      .filter((item) =>
        `${item.label} ${item.detail}`.toLowerCase().includes(needle),
      )
      .slice(0, 12)
  }, [catalog, query])

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        setOpen((value) => !value)
        return
      }
      if (!open) return
      if (event.key === 'Escape') {
        event.preventDefault()
        setOpen(false)
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [open])

  useEffect(() => {
    if (!open) return
    setQuery('')
    setActive(0)
    const id = window.setTimeout(() => inputRef.current?.focus(), 20)
    return () => window.clearTimeout(id)
  }, [open])

  useEffect(() => {
    setActive(0)
  }, [query])

  function go(href: string) {
    setOpen(false)
    router.push(href)
  }

  if (!open) return null

  return (
    <div className="fixed inset-0 z-50">
      <button
        type="button"
        aria-label="Close command palette"
        className="absolute inset-0 bg-[var(--portal-navy)]/40"
        onClick={() => setOpen(false)}
      />
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        className="portal-glass-panel absolute left-1/2 top-[12vh] w-[min(92vw,36rem)] -translate-x-1/2 overflow-hidden rounded-[var(--portal-panel-radius)]"
      >
        <input
          ref={inputRef}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'ArrowDown') {
              event.preventDefault()
              setActive((value) => Math.min(value + 1, Math.max(results.length - 1, 0)))
            } else if (event.key === 'ArrowUp') {
              event.preventDefault()
              setActive((value) => Math.max(value - 1, 0))
            } else if (event.key === 'Enter' && results[active]) {
              event.preventDefault()
              go(results[active].href)
            }
          }}
          placeholder="Search people, deals, pages…"
          className="w-full border-b border-[var(--portal-panel-border)] bg-transparent px-4 py-3 text-sm font-light text-[var(--portal-text)] outline-none placeholder:text-black/35"
        />
        <ul className="max-h-[min(60vh,22rem)] overflow-y-auto p-1.5">
          {results.length === 0 ? (
            <li className="px-3 py-6 text-sm font-light text-black/40">
              No matches.
            </li>
          ) : (
            results.map((item, index) => (
              <li key={item.id}>
                <button
                  type="button"
                  onMouseEnter={() => setActive(index)}
                  onClick={() => go(item.href)}
                  className={[
                    'flex min-h-11 w-full items-center justify-between gap-3 rounded-[var(--portal-tab-radius)] px-3 text-left',
                    index === active
                      ? 'bg-[var(--portal-navy)] text-white'
                      : 'text-[var(--portal-navy)] hover:bg-[var(--portal-rail-hover-bg)]',
                  ].join(' ')}
                >
                  <span className="min-w-0 truncate text-sm font-medium">
                    {item.label}
                  </span>
                  <span
                    className={[
                      'shrink-0 text-[10px] font-light uppercase tracking-[0.12em]',
                      index === active ? 'text-white/70' : 'text-black/40',
                    ].join(' ')}
                  >
                    {item.group}
                    {item.detail && item.group !== 'Go to' ? ` · ${item.detail}` : ''}
                  </span>
                </button>
              </li>
            ))
          )}
        </ul>
      </div>
    </div>
  )
}

export function CommandPaletteHint() {
  return (
    <span className="hidden text-[10px] font-light uppercase tracking-[0.16em] text-[var(--portal-ivory)]/45 xl:inline">
      ⌘K
    </span>
  )
}
